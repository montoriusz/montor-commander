# Source the user's normal interactive config first
if [ -f ~/.zshrc ]; then . ~/.zshrc; fi

__osc133_aid_counter=0
__osc133_prompt_start() { printf '\033]133;A;aid=%s-%s\007' "$$" "$__osc133_aid_counter"; }
__osc133_prompt_end()   { printf '\033]133;B;aid=%s-%s\007' "$$" "$__osc133_aid_counter"; }
__osc133_cmd_start()    { printf '\033]133;C;aid=%s-%s\007' "$$" "$__osc133_aid_counter"; }
__osc133_cmd_done() {
    local ec=$?
    if [[ $__osc133_aid_counter != 0 ]]; then
        printf '\033]133;D;%s;aid=%s-%s\007' "$ec" "$$" "$__osc133_aid_counter"
    fi
    __osc133_aid_counter=$((__osc133_aid_counter + 1))
}

# precmd runs before the prompt is shown -> emit D (finished).
# Must capture $? first.
__osc133_precmd() {
    __osc133_cmd_done
}

# preexec runs after a command is read but before it executes -> C marker.
__osc133_preexec() {
    __osc133_cmd_start
}

autoload -Uz add-zsh-hook
add-zsh-hook precmd __osc133_precmd
add-zsh-hook preexec __osc133_preexec

# Wrap PS1 with prompt-start (A) and prompt-end (B) markers.
# %{...%} is zsh's zero-width escape (equivalent to bash \[ \]).
PROMPT='%{$(__osc133_prompt_start)%}'"$PROMPT"'%{$(__osc133_prompt_end)%}'
