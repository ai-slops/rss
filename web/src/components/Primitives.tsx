import type { Icon } from "@phosphor-icons/react"
import type { ButtonHTMLAttributes, PropsWithChildren, ReactNode } from "react"

export type Tone = "neutral" | "preview" | "program" | "danger" | "healthy" | "warning" | "info"

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  readonly tone?: Tone
  readonly pending?: boolean
  readonly icon?: Icon
}

export function Button({ tone = "neutral", pending = false, icon: IconComponent, children, ...props }: ButtonProps) {
  return (
    <button className={`button button--${tone}`} type="button" {...props}>
      {IconComponent === undefined ? null : <IconComponent aria-hidden="true" weight="bold" />}
      <span>{pending ? "Working..." : children}</span>
    </button>
  )
}

export function IconButton({ "aria-label": label, icon: IconComponent, tone = "neutral", ...props }: Omit<ButtonProps, "children"> & { readonly "aria-label": string; readonly icon: Icon }) {
  return (
    <button aria-label={label} className={`icon-button button--${tone}`} type="button" {...props}>
      <IconComponent aria-hidden="true" weight="bold" />
    </button>
  )
}

export function StatusChip({ tone, children }: PropsWithChildren<{ readonly tone: Tone }>) {
  return <span className={`status-chip status-chip--${tone}`}>{children}</span>
}

export function ScopeTag({ scope }: { readonly scope: "DRAFT" | "LIVE / SHARED" | "THIS BROWSER" }) {
  const tone = scope === "DRAFT" ? "preview" : scope === "LIVE / SHARED" ? "warning" : "info"
  return <span className={`scope-tag scope-tag--${tone}`}>{scope}</span>
}

type PanelProps = PropsWithChildren<{
  readonly title: string
  readonly scope?: "DRAFT" | "LIVE / SHARED" | "THIS BROWSER"
  readonly actions?: ReactNode
  readonly className?: string
}>

export function Panel({ title, scope, actions, className = "", children }: PanelProps) {
  return (
    <section className={`panel ${className}`}>
      <header className="panel__header">
        <div className="cluster">
          <h2>{title}</h2>
          {scope === undefined ? null : <ScopeTag scope={scope} />}
        </div>
        {actions}
      </header>
      <div className="panel__body">{children}</div>
    </section>
  )
}

export function Field({ label, children, hint }: PropsWithChildren<{ readonly label: string; readonly hint?: string }>) {
  return (
    <fieldset className="field">
      <legend className="field__label">{label}</legend>
      {children}
      {hint === undefined ? null : <span className="field__hint">{hint}</span>}
    </fieldset>
  )
}

export function EmptyState({ title, detail, action }: { readonly title: string; readonly detail: string; readonly action?: ReactNode }) {
  return (
    <div className="empty-state">
      <strong>{title}</strong>
      <p>{detail}</p>
      {action}
    </div>
  )
}

export function Notice({ tone, title, children }: PropsWithChildren<{ readonly tone: Tone; readonly title: string }>) {
  return (
    <div className={`notice notice--${tone}`} role={tone === "danger" ? "alert" : "status"}>
      <strong>{title}</strong>
      <span>{children}</span>
    </div>
  )
}
