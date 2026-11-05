export function diagnosticOmissions(value = "") {
  const allowed = new Set(["freezePublisher", "obsProcessSigstop"])
  const omitted = value === "" ? [] : value.split(",")
  if (omitted.some(kind => !allowed.has(kind)) || new Set(omitted).size !== omitted.length) {
    throw new Error("Diagnostic omissions must name distinct supported native cases")
  }
  return omitted
}

export function recoveryDisposition(omitted) {
  return { status: omitted.length === 0 ? "pass" : "partial", obsClaim: omitted.length === 0,
    diagnosticOmissions: [...omitted] }
}
