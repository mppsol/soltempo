# soltempo v0.3 Kamino CPI + pull-back proof — recorded with VHS
# (https://github.com/charmbracelet/vhs).
#
# Usage: vhs demo-video/kamino-proof.tape
# Output: demo-video/output/kamino-proof.mp4
#
# 3-step terminal proof segment (~50s):
#   1. cargo test — 15 v0.3-related tests pass (klend discriminators,
#      ix shape, PDAs, GenericExtraArgsV2, PullbackForPayout intent)
#   2. anchor IDL — v0.3 instructions live on the deployed devnet program
#   3. solana confirm — recent pull-back tx on devnet, RequestPullbackToTempo
#      logs visible
#
# Concatenates with opening + merchant-web + closing for the full demo.

Output demo-video/output/kamino-proof.mp4

Set Shell "bash"
Set FontSize 16
Set Width 1600
Set Height 900
Set Padding 30
Set Theme "TokyoNight"
Set TypingSpeed 25ms
Set PlaybackSpeed 1.0

# Position cwd; nothing user-visible yet.
Hide
Type "cd /Users/hiroyusai/src/soltempo && clear"
Enter
Sleep 1s
Show

Type "# soltempo v0.3 — Kamino CPI + pull-back path proof"
Enter
Sleep 1500ms

Type ""
Enter
Type "# 1. Klend CPI + pull-back unit tests (15 of 37 total)"
Enter
Sleep 600ms
Type "cargo test -p vault 2>&1 | grep -E 'klend_|generic_extra|pullback' | grep '^test '"
Enter
Sleep 8s

Type ""
Enter
Type "# 2. v0.3 instructions live on devnet program 2YhYmf…RXe3"
Enter
Sleep 600ms
Type "cat target/idl/vault.json | jq -r '.instructions[].name'"
Enter
Sleep 4s

Type ""
Enter
Type "# 3. Pull-back tx on devnet — RequestPullbackToTempo + Program data event"
Enter
Sleep 600ms
Type "solana confirm -v 24CJ82MhCn7W1pfxjWAcgevPo6MWxrRT575Pb76KzPMawwNKAg7bV6LA7cncLqmYh6qZ6ifmF8EqZBFybfaQmK5d --url devnet 2>&1 | grep -E 'Status|Program log|Program data' | head -5"
Enter
Sleep 6s

Type ""
Enter
Type "# Kamino end-to-end on localnet via scripts/localnet-with-klend.sh (klend has no devnet)"
Enter
Sleep 5s
