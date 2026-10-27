import { Broadcast, LockKey } from "@phosphor-icons/react"
import { type FormEvent, useState } from "react"
import { Button, Notice } from "./Primitives"

type AuthScreenProps = {
  readonly busy: boolean
  readonly error: string | null
  readonly onLogin: (password: string) => Promise<void>
}

export function AuthScreen({ busy, error, onLogin }: AuthScreenProps) {
  const [password, setPassword] = useState("")

  async function submit(event: FormEvent<HTMLFormElement>): Promise<void> {
    event.preventDefault()
    await onLogin(password)
  }

  return (
    <main className="auth-shell">
      <form className="auth-card" onSubmit={(event) => void submit(event)}>
        <div className="auth-mark"><Broadcast aria-hidden="true" weight="duotone" /></div>
        <div>
          <p className="overline">CONTROL SESSION</p>
          <h1>Live Production</h1>
          <p className="muted">Sign in to control this production. Media continues when this browser closes.</p>
        </div>
        {error === null ? null : <Notice tone="danger" title="Sign-in failed">{error}</Notice>}
        <label className="field">
          <span className="field__label">Operator password</span>
          <span className="input-with-icon">
            <LockKey aria-hidden="true" />
            <input autoComplete="current-password" name="password" onChange={(event) => setPassword(event.currentTarget.value)} required type="password" value={password} />
          </span>
        </label>
        <Button disabled={busy || password.length === 0} pending={busy} tone="preview" type="submit">Open control room</Button>
      </form>
    </main>
  )
}
