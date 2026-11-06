export function diagnosticOmissions(value = "") {
  const allowed = new Set(["receiverDataFreeze", "obsProcessSigstop"])
  const omitted = value === "" ? [] : value.split(",")
  if (omitted.some(kind => !allowed.has(kind)) || new Set(omitted).size !== omitted.length) {
    throw new Error("Diagnostic omissions must name distinct supported native cases")
  }
  return omitted
}

export function recoveryDisposition(omitted, unavailable = []) {
  const complete = omitted.length === 0 && unavailable.length === 0
  return { status: complete ? "pass" : "partial", obsClaim: complete,
    diagnosticOmissions: [...omitted], requiredUnavailable: [...unavailable] }
}
