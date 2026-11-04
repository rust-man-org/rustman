# Import and collection deletion feedback

Continue the user's sidebar environment work and preserve existing edits.

- After import has reached local storage, show a bottom-right success toast:
  Import complete, collection name, Collection, Dismiss and success icon.
- Empty/cancelled or failed imports never report completion.
- DeleteCollection requests open a modal; no data changes at this point.
- Show the collection name and actual local request/folder counts. Do not
  invent unsupported fork/monitor/mock-server counts from the reference.
- Require the exact word `delete`; Confirm is disabled until it matches.
  Cancel, close and Escape keep data. Enforce validation in the handler too.
- Delete transactionally. Database errors retain data and the open dialog.
- Only after successful deletion show Collection deleted in a small toast.
- Toasts dismiss manually or after eight seconds. Identity guards prevent
  an older timer from dismissing a newer toast. No idle polling timer.
- Notifications and confirmation input are transient, not session data.

Verification: red/green import/deletion/rollback tests, stale-toast test,
existing feature gates and Playwright reference renders. No commit or push.
