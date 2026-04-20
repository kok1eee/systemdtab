_sdtab() {
    local cur prev words cword
    cur="${COMP_WORDS[COMP_CWORD]}"
    prev="${COMP_WORDS[COMP_CWORD-1]}"
    words=("${COMP_WORDS[@]}")
    cword=$COMP_CWORD

    local subcommands="init add list remove edit logs restart run status enable disable export apply doctor completions"

    if [[ $cword -eq 1 ]]; then
        COMPREPLY=($(compgen -W "$subcommands" -- "$cur"))
        return
    fi

    local cmd="${words[1]}"
    case "$cmd" in
        logs|status|edit|remove|enable|disable|restart|run)
            if [[ "$cur" == -* ]]; then
                case "$cmd" in
                    logs)
                        COMPREPLY=($(compgen -W "-f --follow -n --lines -p --priority --all --failed --since" -- "$cur"))
                        ;;
                esac
                return
            fi
            local names
            names=$(sdtab __names 2>/dev/null)
            COMPREPLY=($(compgen -W "$names" -- "$cur"))
            ;;
        apply|export)
            if [[ "$cur" == -* ]]; then
                case "$cmd" in
                    apply) COMPREPLY=($(compgen -W "--prune --dry-run" -- "$cur")) ;;
                    export) COMPREPLY=($(compgen -W "-o --output" -- "$cur")) ;;
                esac
                return
            fi
            COMPREPLY=($(compgen -f -- "$cur"))
            ;;
        list)
            COMPREPLY=($(compgen -W "--json --sort" -- "$cur"))
            ;;
        init)
            COMPREPLY=($(compgen -W "--slack-webhook --slack-mention" -- "$cur"))
            ;;
        completions)
            COMPREPLY=($(compgen -W "bash zsh fish" -- "$cur"))
            ;;
        add)
            COMPREPLY=($(compgen -W "--name --workdir --description --env-file --restart --memory-max --cpu-quota --io-weight --timeout-stop --exec-start-pre --exec-stop-post --log-level-max --random-delay --env --no-notify --dry-run" -- "$cur"))
            ;;
    esac
}

complete -F _sdtab sdtab
