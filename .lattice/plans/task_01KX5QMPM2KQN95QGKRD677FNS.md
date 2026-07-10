# MA-8: Centralize Gemini model IDs in a models module

BUILDPLAN M2 / WS-4c (BUILDPLAN id MA-08). Create src/lib/models.ts with Gemini model ID constants; migrate all 5 hardcoded call sites; append a no-gemini-literal grep gate to scripts/debt-gates.sh. AC: AC-22. Serialize: handlers/*. Size S.
