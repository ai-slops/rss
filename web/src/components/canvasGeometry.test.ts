import { describe, expect, it } from "vitest"
import type { SceneItem } from "../api/schema"
import {
  advanceCanvasGesture,
  beginCanvasGesture,
  mapClientPointToCanvas,
  shouldCancelCanvasGesture,
  updateItemRect,
} from "./canvasGeometry"

const item: SceneItem = {
  id: "camera-a",
  name: "Camera A",
  visible: true,
  locked: false,
  rect: { x: 100, y: 80, width: 640, height: 360 },
  crop: { left: 0.1, top: 0.2, right: 0.15, bottom: 0.05 },
  fit: "cover",
  opacity: 0.8,
  content: { kind: "track", trackId: "track-a" },
}

describe("canvas letterbox geometry", () => {
  it("maps a landscape canvas through vertical CSS letterboxing", () => {
    const point = mapClientPointToCanvas({
      client: { x: 510, y: 238.75 },
      bounds: { left: 10, top: 20, width: 1_000, height: 1_000 },
      canvas: { width: 1_920, height: 1_080 },
    })

    expect(point.x).toBeCloseTo(960)
    expect(point.y).toBeCloseTo(0)
  })

  it("maps a portrait canvas through horizontal CSS letterboxing", () => {
    const point = mapClientPointToCanvas({
      client: { x: 369.375, y: 270 },
      bounds: { left: 10, top: 20, width: 1_000, height: 500 },
      canvas: { width: 1_080, height: 1_920 },
    })

    expect(point.x).toBeCloseTo(0)
    expect(point.y).toBeCloseTo(960)
  })
})

describe("canvas transform gestures", () => {
  it("moves outside the canvas without changing canonical crop or fit", () => {
    const active = beginCanvasGesture({ pointerId: 7, start: { x: 120, y: 90 }, rect: item.rect, mode: "move" })
    const moved = advanceCanvasGesture(active, { kind: "move", pointerId: 7, point: { x: -80, y: -110 }, aspectLocked: false })
    const pending = updateItemRect(item, moved.rect)

    expect(pending.rect).toEqual({ x: -100, y: -120, width: 640, height: 360 })
    expect(pending.crop).toEqual(item.crop)
    expect(pending.fit).toBe("cover")
  })

  it("clamps a west resize to one canvas pixel with finite geometry", () => {
    const active = beginCanvasGesture({ pointerId: 8, start: { x: 100, y: 80 }, rect: item.rect, mode: "w" })
    const moved = advanceCanvasGesture(active, { kind: "move", pointerId: 8, point: { x: 10_000, y: 80 }, aspectLocked: false })

    expect(moved.rect).toEqual({ x: 739, y: 80, width: 1, height: 360 })
    expect(Object.values(moved.rect).every(Number.isFinite)).toBe(true)
  })

  it("preserves the starting ratio during a southeast resize", () => {
    const active = beginCanvasGesture({ pointerId: 9, start: { x: 740, y: 440 }, rect: item.rect, mode: "se" })
    const moved = advanceCanvasGesture(active, { kind: "move", pointerId: 9, point: { x: 1_060, y: 500 }, aspectLocked: true })

    expect(moved.rect.width / moved.rect.height).toBeCloseTo(16 / 9)
    expect(moved.rect.x).toBe(100)
    expect(moved.rect.y).toBe(80)
  })

  it("keeps both aspect-locked dimensions at least one canvas pixel", () => {
    const active = beginCanvasGesture({ pointerId: 12, start: { x: 100, y: 80 }, rect: item.rect, mode: "w" })
    const moved = advanceCanvasGesture(active, { kind: "move", pointerId: 12, point: { x: 10_000, y: 80 }, aspectLocked: true })

    expect(moved.rect.width).toBeCloseTo(16 / 9)
    expect(moved.rect.height).toBe(1)
  })

  it("emits one commit for one completed gesture", () => {
    const active = beginCanvasGesture({ pointerId: 10, start: { x: 100, y: 80 }, rect: item.rect, mode: "move" })
    const first = advanceCanvasGesture(active, { kind: "finish", pointerId: 10, point: { x: 110, y: 95 }, aspectLocked: false })
    const duplicate = advanceCanvasGesture(first.state, { kind: "finish", pointerId: 10, point: { x: 110, y: 95 }, aspectLocked: false })

    expect(first.commit).toEqual({ x: 110, y: 95, width: 640, height: 360 })
    expect(duplicate.commit).toBeNull()
  })

  it("cancels pointer and Escape gestures without a commit", () => {
    const active = beginCanvasGesture({ pointerId: 11, start: { x: 100, y: 80 }, rect: item.rect, mode: "move" })
    const cancelled = advanceCanvasGesture(active, { kind: "cancel", pointerId: 11 })

    expect(cancelled.commit).toBeNull()
    expect(cancelled.state.kind).toBe("idle")
    expect(shouldCancelCanvasGesture({ key: "Escape", isComposing: false, isTypingTarget: false, isModalTarget: false })).toBe(true)
  })

  it("does not intercept Escape during typing, modal use, or IME composition", () => {
    expect(shouldCancelCanvasGesture({ key: "Escape", isComposing: true, isTypingTarget: false, isModalTarget: false })).toBe(false)
    expect(shouldCancelCanvasGesture({ key: "Escape", isComposing: false, isTypingTarget: true, isModalTarget: false })).toBe(false)
    expect(shouldCancelCanvasGesture({ key: "Escape", isComposing: false, isTypingTarget: false, isModalTarget: true })).toBe(false)
  })
})
