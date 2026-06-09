# Source the user's normal interactive config first
if [ -f ~/.bashrc ]; then . ~/.bashrc; fi

__osc133_id=0
__osc133_prompt_start() { printf '\033]133;A;aid=%s\007' "$__osc133_id"; }
__osc133_prompt_end()   { printf '\033]133;B\007' "$__osc133_id"; }
__osc133_cmd_start()    { printf '\033]133;C\007' "$__osc133_id"; }
__osc133_cmd_done() {
    local ec=$?
    if [[ $__osc133_id != 0 ]]; then
        printf '\033]133;D;%s;aid=%s\007' "$ec" "$__osc133_id"
    fi
    __osc133_id=$((__osc133_id + 1))
}


# PROMPT_COMMAND runs before PS1 is shown; capture $? first so other entries
# don't clobber it, then emit D (finished) and A (prompt start).
PROMPT_COMMAND='__osc133_cmd_done; '"${PROMPT_COMMAND:-}"

# PS0 is printed after a command is read but before it executes -> C marker.
PS0='$(__osc133_cmd_start)'"${PS0:-}"
PS1='\[$(__osc133_prompt_start)\]'"$PS1"'\[$(__osc133_prompt_end)\]'
