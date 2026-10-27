import type { SceneItem } from "../api/schema"

export type CanvasPoint = { readonly x: number; readonly y: number }
export type CanvasRect = { readonly x: number; readonly y: number; readonly width: number; readonly height: number }
export type CanvasSize = { readonly width: number; readonly height: number }
export type ResizeHandle = "n" | "ne" | "e" | "se" | "s" | "sw" | "w" | "nw"
export type GestureMode = "move" | ResizeHandle

type ActiveGesture = {
  readonly kind: "active"
  readonly pointerId: number
  readonly start: CanvasPoint
  readonly startRect: CanvasRect
  readonly rect: CanvasRect
  readonly mode: GestureMode
}

type IdleGesture = { readonly kind: "idle"; readonly rect: CanvasRect }
export type CanvasGestureState = ActiveGesture | IdleGesture

type MoveAction = {
  readonly kind: "move" | "finish"
  readonly pointerId: number
  readonly point: CanvasPoint
  readonly aspectLocked: boolean
}

type CancelAction = { readonly kind: "cancel"; readonly pointerId: number }
export type CanvasGestureAction = MoveAction | CancelAction

export type CanvasGestureTransition = {
  readonly state: CanvasGestureState
  readonly rect: CanvasRect
  readonly commit: CanvasRect | null
}

type HandleVector = { readonly x: -1 | 0 | 1; readonly y: -1 | 0 | 1 }

const HANDLE_VECTORS: Readonly<Record<ResizeHandle, HandleVector>> = {
  n: { x: 0, y: -1 },
  ne: { x: 1, y: -1 },
  e: { x: 1, y: 0 },
  se: { x: 1, y: 1 },
  s: { x: 0, y: 1 },
  sw: { x: -1, y: 1 },
  w: { x: -1, y: 0 },
  nw: { x: -1, y: -1 },
}

function assertNever(value: never): never {
  throw new TypeError(`Unhandled canvas gesture variant: ${String(value)}`)
}

function finiteDelta(value: number): number {
  return Number.isFinite(value) ? value : 0
}

function resizedRect(input: {
  readonly rect: CanvasRect
  readonly mode: ResizeHandle
  readonly delta: CanvasPoint
  readonly aspectLocked: boolean
}): CanvasRect {
  const { rect, mode, delta, aspectLocked } = input
  const vector = HANDLE_VECTORS[mode]
  const width = Math.max(1, rect.width + vector.x * finiteDelta(delta.x))
  const height = Math.max(1, rect.height + vector.y * finiteDelta(delta.y))
  let nextWidth = width
  let nextHeight = height

  if (aspectLocked) {
    const ratio = rect.width / rect.height
    if (vector.x !== 0 && vector.y !== 0) {
      if (Math.abs(width - rect.width) >= Math.abs(height - rect.height) * ratio) nextHeight = width / ratio
      else nextWidth = height * ratio
    } else if (vector.x !== 0) {
      nextHeight = width / ratio
    } else {
      nextWidth = height * ratio
    }
    if (nextWidth < 1 || nextHeight < 1) {
      if (ratio >= 1) {
        nextWidth = ratio
        nextHeight = 1
      } else {
        nextWidth = 1
        nextHeight = 1 / ratio
      }
    }
  }

  const right = rect.x + rect.width
  const bottom = rect.y + rect.height
  const centerX = rect.x + rect.width / 2
  const centerY = rect.y + rect.height / 2
  const x = vector.x < 0 ? right - nextWidth : vector.x > 0 ? rect.x : centerX - nextWidth / 2
  const y = vector.y < 0 ? bottom - nextHeight : vector.y > 0 ? rect.y : centerY - nextHeight / 2
  return { x, y, width: nextWidth, height: nextHeight }
}

function gestureRect(gesture: ActiveGesture, point: CanvasPoint, aspectLocked: boolean): CanvasRect {
  const delta = {
    x: finiteDelta(point.x - gesture.start.x),
    y: finiteDelta(point.y - gesture.start.y),
  }
  if (gesture.mode === "move") {
    return {
      ...gesture.startRect,
      x: gesture.startRect.x + delta.x,
      y: gesture.startRect.y + delta.y,
    }
  }
  return resizedRect({ rect: gesture.startRect, mode: gesture.mode, delta, aspectLocked })
}

export function mapClientPointToCanvas(input: {
  readonly client: CanvasPoint
  readonly bounds: { readonly left: number; readonly top: number; readonly width: number; readonly height: number }
  readonly canvas: CanvasSize
}): CanvasPoint {
  const scale = Math.min(input.bounds.width / input.canvas.width, input.bounds.height / input.canvas.height)
  if (!Number.isFinite(scale) || scale <= 0) return { x: 0, y: 0 }
  const renderedWidth = input.canvas.width * scale
  const renderedHeight = input.canvas.height * scale
  const left = input.bounds.left + (input.bounds.width - renderedWidth) / 2
  const top = input.bounds.top + (input.bounds.height - renderedHeight) / 2
  return {
    x: finiteDelta((input.client.x - left) / scale),
    y: finiteDelta((input.client.y - top) / scale),
  }
}

export function beginCanvasGesture(input: {
  readonly pointerId: number
  readonly start: CanvasPoint
  readonly rect: CanvasRect
  readonly mode: GestureMode
}): CanvasGestureState {
  return { kind: "active", pointerId: input.pointerId, start: input.start, startRect: input.rect, rect: input.rect, mode: input.mode }
}

export function advanceCanvasGesture(state: CanvasGestureState, action: CanvasGestureAction): CanvasGestureTransition {
  if (state.kind === "idle" || state.pointerId !== action.pointerId) return { state, rect: state.rect, commit: null }
  switch (action.kind) {
    case "move": {
      const rect = gestureRect(state, action.point, action.aspectLocked)
      const next = { ...state, rect }
      return { state: next, rect, commit: null }
    }
    case "finish": {
      const rect = gestureRect(state, action.point, action.aspectLocked)
      return { state: { kind: "idle", rect }, rect, commit: rect }
    }
    case "cancel":
      return { state: { kind: "idle", rect: state.startRect }, rect: state.startRect, commit: null }
    default:
      return assertNever(action)
  }
}

export function updateItemRect(item: SceneItem, rect: CanvasRect): SceneItem {
  return { ...item, rect }
}

export function shouldCancelCanvasGesture(input: {
  readonly key: string
  readonly isComposing: boolean
  readonly isTypingTarget: boolean
  readonly isModalTarget: boolean
}): boolean {
  return input.key === "Escape" && !input.isComposing && !input.isTypingTarget && !input.isModalTarget
}
