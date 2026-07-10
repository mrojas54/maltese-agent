# MA-12: Deduplicate propose handlers via factory

BUILDPLAN M3 / WS-11 (BUILDPLAN id MA-12). makeProposeFixHandler factory parameterizing prompt file and model; the three propose* handlers become instances; parity tests (existing plus new) prove identical behavior per kind. AC: AC-24. Depends: MA-08, MA-10. Serialize: handlers/*. Size M.
