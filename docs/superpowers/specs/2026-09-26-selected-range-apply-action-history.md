# Selected-range Apply and Action History access

## Behavior

- Applying an engineering value with more than one table cell selected writes
  that value to the whole highlighted rectangle in one BIN transaction. Keep
  the range selected, retain the entered value, and make the operation undoable
  as one history item.
- A single selected cell continues to use the existing cell-edit path. Axis
  selection continues to edit only its selected axis element. Raw-byte editing
  remains unchanged.
- Add an **Action History…** command to Edit. It opens the existing Action
  History window for the active table, reusing its current history and dock
  behavior; it does not introduce a second history store.

## Validation

- Test a multi-cell engineering Apply changes every selected cell, leaves other
  cells untouched, retains the selection, and undoes as one action.
- Test the Edit menu exposes Action History and opens the existing per-table
  window when a table is active.
