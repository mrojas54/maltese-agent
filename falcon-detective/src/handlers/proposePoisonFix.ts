// Thin re-export (WS-11 / AC-24): the handler instance is created by
// makeProposeFixHandler and MUST live in proposeFix.ts — Barnum resolves
// handlers by the module that called createHandler (see proposeFix.ts's
// module doc). This file keeps the pre-factory import path stable.
export { proposePoisonFix } from "./proposeFix.js";
