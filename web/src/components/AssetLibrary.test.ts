import { describe, expect, it } from "vitest"
import type { Scene } from "../api/schema"
import { appendStaticPlacement, validateImageUpload } from "./AssetLibrary"

const scene: Scene = { id: "scene-a", name: "Standby", revision: "4", items: [] }

describe("asset and static placement contracts", () => {
  it("accepts PNG or JPEG files only within four MiB", () => {
    expect(validateImageUpload({ name: "still.png", type: "image/png", size: 4_194_304 })).toEqual({ kind: "valid" })
    expect(validateImageUpload({ name: "still.gif", type: "image/gif", size: 100 }).kind).toBe("invalid")
    expect(validateImageUpload({ name: "large.jpg", type: "image/jpeg", size: 4_194_305 }).kind).toBe("invalid")
  })

  it("adds an image using the exact saved asset identity and version", () => {
    const next = appendStaticPlacement({
      scene,
      itemId: "item-a",
      name: "Holding slide",
      canvas: { width: 1_920, height: 1_080 },
      content: { kind: "image", assetId: "asset-a", version: "3" },
    })

    expect(next.items[0]?.content).toEqual({ kind: "image", assetId: "asset-a", version: "3" })
    expect(next.items[0]?.rect).toEqual({ x: 0, y: 0, width: 1_920, height: 1_080 })
  })

  it("adds basic text and solid content without source identities", () => {
    const text = appendStaticPlacement({ scene, itemId: "text-a", name: "Title", canvas: { width: 960, height: 540 }, content: { kind: "text", text: "Starting soon", color: "#ffffff", fontSize: 48 } })
    const solid = appendStaticPlacement({ scene, itemId: "solid-a", name: "Background", canvas: { width: 960, height: 540 }, content: { kind: "solid", color: "#10161b" } })

    expect(text.items[0]?.content.kind).toBe("text")
    expect(solid.items[0]?.content.kind).toBe("solid")
  })
})
