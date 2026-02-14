#!/bin/bash
# smc relay launcher — starts two Claude instances in tmux

SESSION="smc-relay"

# Kill existing session if any
tmux kill-session -t "$SESSION" 2>/dev/null

# Create session with first pane
tmux new-session -d -s "$SESSION" -n "relay"

# Enable mouse (click to switch panes, drag to resize, scroll)
tmux set -g mouse on

# Split horizontally
tmux split-window -h -t "$SESSION"

# Get pane IDs
PANE_LEFT=$(tmux list-panes -t "$SESSION" -F '#{pane_id}' | head -1)
PANE_RIGHT=$(tmux list-panes -t "$SESSION" -F '#{pane_id}' | tail -1)

# Register instances
smc relay register claude-left --pane "$PANE_LEFT"
smc relay register claude-right --pane "$PANE_RIGHT"

echo ""
echo "Relay ready!"
echo "  claude-left  -> $PANE_LEFT"
echo "  claude-right -> $PANE_RIGHT"
echo ""
echo "Attaching to tmux session '$SESSION'..."
echo "Start claude in each pane, then tell each one their name."
echo ""

# Attach
tmux attach -t "$SESSION"
