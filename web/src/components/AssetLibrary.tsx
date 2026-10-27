import { FileArrowUp, Image, Plus } from "@phosphor-icons/react"
import { useState } from "react"
import { z } from "zod"
import type { Scene, SceneItem, Snapshot } from "../api/schema"
import type { StaticContent } from "./AssetLibraryForms"
import { StaticPlacementForms } from "./AssetLibraryForms"
import { Button, EmptyState, Field, StatusChip } from "./Primitives"
import type { OperatorCommand, OperatorCommandReceipt } from "./ProductionSetup"

const MAXIMUM_IMAGE_BYTES = 4_194_304
const imageUploadSchema = z.object({
  name: z.string().min(1),
  type: z.enum(["image/png", "image/jpeg"], "Choose a static PNG or JPEG image."),
  size: z.number().int().positive("Choose a non-empty image file.").max(MAXIMUM_IMAGE_BYTES, "Choose an image no larger than 4 MiB."),
})

export type AssetLibraryProps = {
  readonly snapshot: Snapshot
  readonly disabled: boolean
  readonly onCommand: OperatorCommand
}

export type ImageUploadMetadata = { readonly name: string; readonly type: string; readonly size: number }
export type ImageUploadValidation = { readonly kind: "valid" } | { readonly kind: "invalid"; readonly message: string }

export function validateImageUpload(file: ImageUploadMetadata): ImageUploadValidation {
  const parsed = imageUploadSchema.safeParse(file)
  return parsed.success ? { kind: "valid" } : { kind: "invalid", message: parsed.error.issues[0]?.message ?? "Review the selected image." }
}

export function appendStaticPlacement(input: {
  readonly scene: Scene
  readonly itemId: string
  readonly name: string
  readonly canvas: { readonly width: number; readonly height: number }
  readonly content: StaticContent
}): Scene {
  const item: SceneItem = {
    id: input.itemId,
    name: input.name,
    visible: true,
    locked: false,
    rect: { x: 0, y: 0, width: input.canvas.width, height: input.canvas.height },
    crop: { left: 0, top: 0, right: 0, bottom: 0 },
    fit: "contain",
    opacity: 1,
    content: input.content,
  }
  return { ...input.scene, items: [...input.scene.items, item] }
}

async function fileBase64(file: File): Promise<string> {
  return new Promise((resolve, reject) => {
    const reader = new FileReader()
    reader.onerror = () => reject(new TypeError("Unable to read the selected image."))
    reader.onload = () => {
      if (typeof reader.result !== "string") {
        reject(new TypeError("Unable to encode the selected image."))
        return
      }
      const separator = reader.result.indexOf(",")
      if (separator < 0) {
        reject(new TypeError("Unable to encode the selected image."))
        return
      }
      resolve(reader.result.slice(separator + 1))
    }
    reader.readAsDataURL(file)
  })
}

function receiptError(receipt: OperatorCommandReceipt | null): string | null {
  if (receipt === null) return "The command did not complete. Refresh before retrying."
  if (receipt.status === "rejected" || receipt.status === "failed") return receipt.reason ?? "The server rejected the command."
  return null
}

export function AssetLibrary({ snapshot, disabled, onCommand }: AssetLibraryProps) {
  const [file, setFile] = useState<File | null>(null)
  const [assetName, setAssetName] = useState("")
  const [pendingUpload, setPendingUpload] = useState(false)
  const [message, setMessage] = useState<string | null>(null)
  const scene = snapshot.project.scenes.find((candidate) => candidate.id === snapshot.preview.sceneId)

  const savePlacement = async (name: string, content: StaticContent): Promise<void> => {
    if (scene === undefined) throw new TypeError("The Preview scene is unavailable. Refresh before adding content.")
    if (name.length === 0) throw new TypeError("Enter a layer name.")
    const nextScene = appendStaticPlacement({ scene, itemId: crypto.randomUUID(), name, canvas: snapshot.project.canvas, content })
    const receipt = await onCommand("scene.save", scene.id, { scene: nextScene }, scene.revision)
    const failure = receiptError(receipt)
    setMessage(failure ?? `${name} added to the Preview draft.`)
    if (failure !== null) throw new TypeError(failure)
  }

  return (
    <div className="production-setup__stack">
      <section className="production-setup__section">
        <header><div><h3>Upload image asset</h3><p>Authenticated PNG/JPEG uploads are limited to 4 MiB and saved as versioned project assets.</p></div></header>
        <form className="production-setup__upload" onSubmit={(event) => {
          event.preventDefault()
          if (file === null) return
          const validation = validateImageUpload(file)
          if (validation.kind === "invalid") { setMessage(validation.message); return }
          setPendingUpload(true)
          void fileBase64(file).then((dataBase64) => onCommand("asset.upload", snapshot.project.id, { name: assetName.trim() || file.name, mime: file.type, dataBase64 }, snapshot.project.revision)).then((receipt) => {
            const failure = receiptError(receipt)
            setMessage(failure ?? "Image uploaded. It is available after the authoritative snapshot refreshes.")
          }).catch((caught: unknown) => {
            if (caught instanceof Error) setMessage(caught.message)
            else throw caught
          }).finally(() => setPendingUpload(false))
        }}>
          <Field label="Image file"><input accept="image/png,image/jpeg" disabled={disabled || pendingUpload} onChange={(event) => { const selected = event.currentTarget.files?.item(0) ?? null; setFile(selected); setAssetName(selected?.name ?? "") }} required type="file" /></Field>
          <Field label="Asset name"><input disabled={disabled || pendingUpload} maxLength={100} onChange={(event) => setAssetName(event.currentTarget.value)} required value={assetName} /></Field>
          <Button disabled={disabled || pendingUpload || file === null} icon={FileArrowUp} pending={pendingUpload} tone="preview" type="submit">Upload image</Button>
        </form>
      </section>
      <section className="production-setup__section"><header><div><h3>Saved images</h3><p>Adds a new placement using the exact saved asset ID and version. Server storage paths remain private.</p></div></header>{snapshot.project.assets.length === 0 ? <EmptyState detail="Upload a PNG or JPEG to add it to a scene." title="No image assets" /> : <div className="production-setup__asset-grid">{snapshot.project.assets.map((asset) => <article className="production-setup__card" key={`${asset.id}-${asset.version}`}><div className="production-setup__card-header"><Image aria-hidden="true" /><div className="row-copy"><strong>{asset.name}</strong><small>{asset.mime}</small></div><StatusChip tone="neutral">VERSION {asset.version}</StatusChip></div><Button disabled={disabled || scene === undefined} icon={Plus} onClick={() => void savePlacement(asset.name, { kind: "image", assetId: asset.id, version: asset.version })} tone="preview">Add image to Preview</Button></article>)}</div>}</section>
      <section className="production-setup__section"><header><div><h3>Built-in content</h3><p>Solid and text placements are saved directly into the selected Preview draft.</p></div></header><StaticPlacementForms disabled={disabled || scene === undefined} onAdd={savePlacement} /></section>
      {message === null ? null : <p aria-live="polite" className="production-setup__message">{message}</p>}
    </div>
  )
}
