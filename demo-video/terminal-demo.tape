# soltempo terminal demo — recorded with VHS (https://github.com/charmbracelet/vhs)
#
# Usage: vhs demo-video/terminal-demo.tape
# Output: demo-video/output/terminal-demo.mp4
#
# Runs scripts/recording-demo.sh which boots the keeper, runs the
# Tempo deposit + bridge, waits for the keeper to relay, and verifies
# the Solana-side state. Total recording duration ~80s.

Output demo-video/output/terminal-demo.mp4

Set Shell "bash"
Set FontSize 18
Set Width 1600
Set Height 900
Set Padding 30
Set Theme "TokyoNight"
Set TypingSpeed 25ms
Set PlaybackSpeed 1.0

# Move to the repo root so paths in scripts work.
Hide
Type "cd /Users/hiroyusai/src/soltempo && export TEMPO_KEEPER_PRIVATE_KEY=0x36e4518b87c5b9f5f875f139ed13da772101e17c9357cc537dc9b899fbc7f6d9 && clear"
Enter
Sleep 1s
Show

# Visible intro line
Type "# soltempo — cross-VM demo: Tempo Moderato → Solana devnet"
Enter
Sleep 1500ms

Type "./scripts/recording-demo.sh"
Enter

# Recording-demo runs ~75s end to end (18s keeper boot + ~10s cast sends
# + 22s relay wait + state checks). Sleep generously so the recording
# captures the full STAGE 5 verification before VHS stops.
Sleep 105s
