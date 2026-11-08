use std::{
    ops::ControlFlow,
    sync::{Arc, Mutex},
};

use gst::prelude::*;
use gstreamer as gst;
use gstreamer::buffer::BufferMetaForeachAction;
use gstreamer_app as gst_app;

struct Fixture {
    producer: gst::Pipeline,
    consumer: gst::Pipeline,
    output: gst_app::AppSink,
    producer_memory: Arc<Mutex<Option<Vec<gst::Memory>>>>,
}

impl Fixture {
    fn new() -> anyhow::Result<Self> {
        gst::init()?;
        let channel = format!("intervideo-identity-{}", uuid::Uuid::now_v7());
        let producer = gst::parse::launch(&format!(
            "videotestsrc is-live=false num-buffers=1 pattern=white ! \
         video/x-raw,format=I420,width=16,height=16,framerate=30/1 ! \
         identity name=producer ! intervideosink channel={channel} sync=false async=false"
        ))?
        .downcast::<gst::Pipeline>()
        .map_err(|_| anyhow::anyhow!("producer pipeline"))?;
        let consumer = gst::parse::launch(&format!(
            "intervideosrc channel={channel} timeout=1000000000 num-buffers=1 ! \
         video/x-raw,format=I420,width=16,height=16,framerate=30/1 ! \
         identity name=transport ! appsink name=output sync=false"
        ))?
        .downcast::<gst::Pipeline>()
        .map_err(|_| anyhow::anyhow!("consumer pipeline"))?;
        let producer_memory = Arc::new(Mutex::new(None::<Vec<gst::Memory>>));
        let observed_memory = Arc::clone(&producer_memory);
        producer
            .by_name("producer")
            .and_then(|element| element.static_pad("src"))
            .ok_or_else(|| anyhow::anyhow!("producer pad"))?
            .add_probe(gst::PadProbeType::BUFFER, move |_, info| {
                if let Some(buffer) = info.buffer_mut() {
                    let reference = gst::Caps::builder("timestamp/x-live-production-engine")
                        .field("continuity-generation", 1_u64)
                        .build();
                    let buffer = buffer.make_mut();
                    gst::ReferenceTimestampMeta::add(
                        buffer,
                        &reference,
                        buffer.pts().unwrap_or(gst::ClockTime::ZERO),
                        buffer.duration(),
                    );
                    if let Ok(mut memory) = observed_memory.lock() {
                        *memory = Some(
                            (0..buffer.n_memory())
                                .map(|index| buffer.peek_memory(index).to_owned())
                                .collect(),
                        );
                    }
                }
                gst::PadProbeReturn::Ok
            })
            .ok_or_else(|| anyhow::anyhow!("producer probe"))?;
        consumer
            .by_name("transport")
            .and_then(|element| element.static_pad("src"))
            .ok_or_else(|| anyhow::anyhow!("transport pad"))?
            .add_probe(gst::PadProbeType::BUFFER, |_, info| {
                if let Some(buffer) = info.buffer_mut() {
                    buffer.make_mut().foreach_meta_mut(|mut meta| {
                        let remove = meta.downcast_ref::<gst::ReferenceTimestampMeta>().is_some();
                        ControlFlow::Continue(if remove {
                            BufferMetaForeachAction::Remove
                        } else {
                            BufferMetaForeachAction::Keep
                        })
                    });
                }
                gst::PadProbeReturn::Ok
            })
            .ok_or_else(|| anyhow::anyhow!("strip probe"))?;
        let output = consumer
            .by_name("output")
            .and_downcast::<gst_app::AppSink>()
            .ok_or_else(|| anyhow::anyhow!("appsink"))?;
        Ok(Self {
            producer,
            consumer,
            output,
            producer_memory,
        })
    }

    fn run(&self) -> anyhow::Result<(Vec<gst::Memory>, gst::Buffer)> {
        self.producer.set_state(gst::State::Playing)?;
        let message = self
            .producer
            .bus()
            .ok_or_else(|| anyhow::anyhow!("producer bus"))?
            .timed_pop_filtered(
                gst::ClockTime::from_seconds(2),
                &[gst::MessageType::Eos, gst::MessageType::Error],
            )
            .ok_or_else(|| anyhow::anyhow!("producer render deadline"))?;
        if let gst::MessageView::Error(error) = message.view() {
            return Err(anyhow::anyhow!("producer failed: {}", error.error()));
        }
        self.consumer.set_state(gst::State::Playing)?;
        let transported = self
            .output
            .try_pull_sample(gst::ClockTime::from_seconds(2))
            .and_then(|sample| sample.buffer_owned())
            .ok_or_else(|| anyhow::anyhow!("transported frame"))?;
        let retained = self
            .producer_memory
            .lock()
            .map_err(|_| anyhow::anyhow!("producer memory lock"))?
            .clone()
            .ok_or_else(|| anyhow::anyhow!("retained producer memory"))?;
        Ok((retained, transported))
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = self.consumer.set_state(gst::State::Null);
        let _ = self.producer.set_state(gst::State::Null);
    }
}

#[test]
fn intervideo_reports_exact_memory_identity_without_restoring_metadata() -> anyhow::Result<()> {
    // Given
    let fixture = Fixture::new()?;

    // When
    let (retained, transported) = fixture.run()?;

    // Then
    let same_layout = retained.len() == transported.n_memory()
        && retained.iter().enumerate().all(|(index, memory)| {
            gst::MemoryRef::ptr_eq(memory.as_ref(), transported.peek_memory(index))
        });
    eprintln!(
        "intervideo memory identity: same_layout={same_layout} producer={retained:?} transported={:?}",
        (0..transported.n_memory())
            .map(|index| transported.peek_memory(index).to_owned())
            .collect::<Vec<_>>()
    );
    assert_eq!(
        transported
            .iter_meta::<gst::ReferenceTimestampMeta>()
            .count(),
        0
    );
    assert!(!retained.is_empty());
    assert_ne!(transported.n_memory(), 0);
    assert!(
        transported.map_readable()?.as_slice()[0] > 200,
        "must observe the rendered white producer frame, not initial timeout-black"
    );
    Ok(())
}
