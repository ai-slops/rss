export function finiteTransferComplete(source, receiver) {
  if (!source.strictDecoded || !receiver.strictDecoded) return false
  if (!Array.isArray(source.frames) || !Array.isArray(receiver.frames)) return false
  for (const [kind, codec] of [["video", "h264"], ["audio", "aac"]]) {
    const before = source.frames.find(value => value.kind === kind)
    const after = receiver.frames.find(value => value.kind === kind)
    if (before?.codec !== codec || after?.codec !== codec) return false
    const count = Number(before.frames)
    if (!Number.isInteger(count) || count <= 0 || Number(after.frames) !== count) return false
    if (kind === "video" && count !== 150) return false
  }
  return true
}
