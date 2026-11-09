import assert from "node:assert/strict"
import test from "node:test"
import { nativeDependencyError } from "./preflight.mjs"

test("native preflight rejects the actual conflicting Ubuntu SRT variants", () => {
  assert.match(nativeDependencyError(`
    libsrt.so.1.5 => /lib/x86_64-linux-gnu/libsrt.so.1.5 (0x1000)
    libsrt-gnutls.so.1.5 => /lib/x86_64-linux-gnu/libsrt-gnutls.so.1.5 (0x2000)
  `), /Conflicting SRT runtimes/u)
  assert.equal(nativeDependencyError(`
    libsrt-gnutls.so.1.5 => /lib/x86_64-linux-gnu/libsrt-gnutls.so.1.5 (0x2000)
    libsrtp2.so.1 => /lib/x86_64-linux-gnu/libsrtp2.so.1 (0x3000)
  `), null)
})

test("ldd exit zero does not waive a missing native dependency", () => {
  assert.match(nativeDependencyError("libobs.so.0 => not found"), /dependency not found/u)
})
