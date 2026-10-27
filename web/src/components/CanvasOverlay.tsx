import type { PointerEvent as ReactPointerEvent } from "react"
import { useCallback, useEffect, useRef, useState } from "react"
import type { Scene, SceneItem } from "../api/schema"
import type { CanvasGestureState, CanvasPoint, CanvasRect, CanvasSize, GestureMode, ResizeHandle } from "./canvasGeometry"
import {
  advanceCanvasGesture,
  beginCanvasGesture,
  mapClientPointToCanvas,
  shouldCancelCanvasGesture,
  updateItemRect,
} from "./canvasGeometry"
import "./CanvasOverlay.css"

export type CanvasOverlayProps = {
  readonly scene: Scene
  readonly canvas: CanvasSize
  readonly selectedItemId: string | null
  readonly onSelectItem: (id: string | null) => void
  readonly onCommitItem: (item: SceneItem, expectedRevision: string) => void
  readonly onGestureChange: (active: boolean) => void
  readonly disabled: boolean
  readonly aspectLocked: boolean
}

type ActiveItemGesture = { readonly item: SceneItem; readonly revision: string; readonly state: CanvasGestureState; readonly clientStart: CanvasPoint; readonly toCanvas: (client: CanvasPoint) => CanvasPoint; readonly moved: boolean }
type HandleDescriptor = {
  readonly mode: ResizeHandle
  readonly label: string
  readonly cursor: string
  readonly point: (rect: CanvasRect) => CanvasPoint
}

const HANDLES: readonly HandleDescriptor[] = [
  { mode: "nw", label: "Resize top left", cursor: "nwse-resize", point: (rect) => ({ x: rect.x, y: rect.y }) },
  { mode: "n", label: "Resize top", cursor: "ns-resize", point: (rect) => ({ x: rect.x + rect.width / 2, y: rect.y }) },
  { mode: "ne", label: "Resize top right", cursor: "nesw-resize", point: (rect) => ({ x: rect.x + rect.width, y: rect.y }) },
  { mode: "e", label: "Resize right", cursor: "ew-resize", point: (rect) => ({ x: rect.x + rect.width, y: rect.y + rect.height / 2 }) },
  { mode: "se", label: "Resize bottom right", cursor: "nwse-resize", point: (rect) => ({ x: rect.x + rect.width, y: rect.y + rect.height }) },
  { mode: "s", label: "Resize bottom", cursor: "ns-resize", point: (rect) => ({ x: rect.x + rect.width / 2, y: rect.y + rect.height }) },
  { mode: "sw", label: "Resize bottom left", cursor: "nesw-resize", point: (rect) => ({ x: rect.x, y: rect.y + rect.height }) },
  { mode: "w", label: "Resize left", cursor: "ew-resize", point: (rect) => ({ x: rect.x, y: rect.y + rect.height / 2 }) },
]
const HANDLE_TARGET_SIZE = 32

function rectsMatch(first: CanvasRect, second: CanvasRect): boolean {
  return first.x === second.x && first.y === second.y && first.width === second.width && first.height === second.height
}

function canvasCoordinates(svg: SVGSVGElement, canvas: CanvasSize): (client: CanvasPoint) => CanvasPoint {
  const matrix = svg.getScreenCTM()?.inverse()
  if (matrix !== undefined) return (client) => new DOMPoint(client.x, client.y).matrixTransform(matrix)
  const bounds = svg.getBoundingClientRect()
  return (client) => mapClientPointToCanvas({ client, bounds, canvas })
}

function isTypingOrModalTarget(target: EventTarget | null): { readonly typing: boolean; readonly modal: boolean } {
  if (!(target instanceof Element)) return { typing: false, modal: false }
  return {
    typing: target.closest("input, textarea, select, [contenteditable='true']") !== null,
    modal: target.closest("dialog, [role='dialog']") !== null,
  }
}

export function CanvasOverlay({ scene, canvas, selectedItemId, onSelectItem, onCommitItem, onGestureChange, disabled, aspectLocked }: CanvasOverlayProps) {
  const svgRef = useRef<SVGSVGElement>(null)
  const activeRef = useRef<ActiveItemGesture | null>(null)
  const [pendingItem, setPendingItem] = useState<SceneItem | null>(null)
  const selectedItem = scene.items.find((item) => item.id === selectedItemId && item.visible)

  const cancelGesture = useCallback((): void => {
    const active = activeRef.current
    if (active === null) return
    const svg = svgRef.current
    if (active.state.kind === "active" && svg?.hasPointerCapture(active.state.pointerId) === true) svg.releasePointerCapture(active.state.pointerId)
    setPendingItem(null)
    activeRef.current = null
    onGestureChange(false)
  }, [onGestureChange])

  useEffect(() => {
    const handleKeyDown = (event: KeyboardEvent): void => {
      if (activeRef.current === null) return
      const target = isTypingOrModalTarget(event.target)
      const modalOpen = target.modal || document.querySelector("dialog[open], [role='dialog'][aria-modal='true']") !== null
      if (!shouldCancelCanvasGesture({ key: event.key, isComposing: event.isComposing, isTypingTarget: target.typing, isModalTarget: modalOpen })) return
      event.preventDefault()
      event.stopImmediatePropagation()
      cancelGesture()
    }
    window.addEventListener("keydown", handleKeyDown, true)
    return () => window.removeEventListener("keydown", handleKeyDown, true)
  }, [cancelGesture])

  useEffect(() => {
    if (!disabled) return
    const active = activeRef.current
    const svg = svgRef.current
    if (active?.state.kind === "active" && svg?.hasPointerCapture(active.state.pointerId) === true) svg.releasePointerCapture(active.state.pointerId)
    activeRef.current = null
    setPendingItem(null)
    onGestureChange(false)
  }, [disabled, onGestureChange])

  useEffect(() => () => onGestureChange(false), [onGestureChange])

  const beginGesture = (item: SceneItem, mode: GestureMode, event: ReactPointerEvent<Element>): void => {
    event.stopPropagation()
    onSelectItem(item.id)
    if (disabled || item.locked || activeRef.current !== null) return
    const svg = svgRef.current
    if (svg === null) return
    svg.focus({ preventScroll: true })
    const toCanvas = canvasCoordinates(svg, canvas)
    const clientStart = { x: event.clientX, y: event.clientY }
    const start = toCanvas(clientStart)
    activeRef.current = { item, revision: scene.revision, state: beginCanvasGesture({ pointerId: event.pointerId, start, rect: item.rect, mode }), clientStart, toCanvas, moved: false }
    onGestureChange(true)
    setPendingItem(item)
    svg.setPointerCapture(event.pointerId)
  }

  const updateGesture = (event: ReactPointerEvent<SVGSVGElement>, finish: boolean): void => {
    const active = activeRef.current
    if (active === null || active.state.kind !== "active" || active.state.pointerId !== event.pointerId) return
    const client = { x: event.clientX, y: event.clientY }
    const moved = active.moved || Math.hypot(client.x - active.clientStart.x, client.y - active.clientStart.y) >= 3
    if (!moved) { if (finish) cancelGesture(); return }
    const point = active.toCanvas(client)
    const transition = advanceCanvasGesture(active.state, {
      kind: finish ? "finish" : "move",
      pointerId: event.pointerId,
      point,
      aspectLocked,
    })
    activeRef.current = { ...active, moved, state: transition.state }
    const nextItem = updateItemRect(active.item, transition.rect)
    setPendingItem(finish ? null : nextItem)
    if (!finish) return
    if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId)
    activeRef.current = null
    onGestureChange(false)
    if (transition.commit !== null && !rectsMatch(active.item.rect, transition.commit)) onCommitItem(nextItem, active.revision)
  }

  const displayItem = pendingItem ?? selectedItem
  return (
    <svg
      aria-label="Preview canvas transform overlay"
      className="canvas-overlay"
      onPointerCancel={(event) => {
        if (activeRef.current === null) return
        if (event.currentTarget.hasPointerCapture(event.pointerId)) event.currentTarget.releasePointerCapture(event.pointerId)
        cancelGesture()
      }}
      onPointerMove={(event) => updateGesture(event, false)}
      onPointerUp={(event) => updateGesture(event, true)}
      preserveAspectRatio="xMidYMid meet"
      ref={svgRef}
      tabIndex={-1}
      viewBox={`0 0 ${canvas.width} ${canvas.height}`}
    >
      <title>Preview canvas transform overlay</title>
      <rect className="canvas-overlay__background" height={canvas.height} onPointerDown={() => onSelectItem(null)} width={canvas.width} x={0} y={0} />
      {scene.items.map((item) => item.visible ? (
        <rect
          aria-label={`Select ${item.name}${item.locked ? ", locked" : ""}`}
          className={`canvas-overlay__hit${item.locked ? " is-locked" : ""}`}
          height={item.rect.height}
          key={item.id}
          onPointerDown={(event) => beginGesture(item, "move", event)}
          width={item.rect.width}
          x={item.rect.x}
          y={item.rect.y}
        />
      ) : null)}
      {displayItem === undefined || displayItem === null ? null : (
        <g aria-label={`${displayItem.name}${pendingItem === null ? " selected" : " pending geometry"}`}>
          <rect className={`canvas-overlay__outline${pendingItem === null ? "" : " is-pending"}`} height={displayItem.rect.height} width={displayItem.rect.width} x={displayItem.rect.x} y={displayItem.rect.y} />
          {pendingItem === null ? null : <text className="canvas-overlay__pending-label" x={displayItem.rect.x} y={displayItem.rect.y}>PENDING</text>}
          {disabled || displayItem.locked ? null : HANDLES.map((handle) => {
            const point = handle.point(displayItem.rect)
            return (
              <foreignObject className="canvas-overlay__handle-shell" height={HANDLE_TARGET_SIZE} key={handle.mode} width={HANDLE_TARGET_SIZE} x={point.x - HANDLE_TARGET_SIZE / 2} y={point.y - HANDLE_TARGET_SIZE / 2}>
                <button aria-label={handle.label} className="canvas-overlay__handle" onPointerDown={(event) => beginGesture(displayItem, handle.mode, event)} style={{ cursor: handle.cursor }} type="button" />
              </foreignObject>
            )
          })}
        </g>
      )}
    </svg>
  )
}
