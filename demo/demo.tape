# Render with: vhs demo/demo.tape  (https://github.com/charmbracelet/vhs)
Output demo.gif
Set FontSize 16
Set Width 1100
Set Height 620
Set Theme "Catppuccin Mocha"
Set TypingSpeed 40ms

Type "# Claude just said: 'Done! Fixed src/auth.ts, all tests pass.'"
Enter
Sleep 1.5s

Type "# stopproof disagrees — it checked:"
Enter
Sleep 1s

Type "cat .stopproof/last-receipt.md"
Enter
Sleep 4s

Type "clear"
Enter

Type "# tests actually failing + a claimed file with no diff = stop BLOCKED"
Enter
Sleep 2s

Type "# the agent keeps working until the evidence exists. Then:"
Enter
Sleep 1.5s

Type "stopproof run"
Enter
Sleep 3.5s

Type "# verdict: PASS — receipts, not vibes."
Enter
Sleep 2.5s
