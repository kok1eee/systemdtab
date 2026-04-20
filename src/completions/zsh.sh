#compdef sdtab

_sdtab() {
    local -a subcommands
    subcommands=(
        'init:Initialize sdtab (enable linger, create directories)'
        'add:Add a new timer or service'
        'list:List all managed timers and services'
        'remove:Remove a timer or service'
        'edit:Edit unit files with $EDITOR'
        'logs:Show logs for a timer or service'
        'restart:Restart a timer or service'
        'run:Trigger a unit once manually (ignores timer schedule)'
        'status:Show detailed status of a timer or service'
        'enable:Enable (start) a timer or service'
        'disable:Disable (stop) a timer or service without removing'
        'export:Export current configuration to TOML'
        'apply:Apply configuration from a TOML file'
        'doctor:Run health checks'
        'completions:Generate shell completion script'
    )

    if (( CURRENT == 2 )); then
        _describe -t commands 'sdtab commands' subcommands
        return
    fi

    local cmd="${words[2]}"

    case "$cmd" in
        logs|status|edit|remove|enable|disable|restart|run)
            if [[ "$cmd" == "logs" ]]; then
                _arguments \
                    '(-f --follow)'{-f,--follow}'[Follow log output]' \
                    '(-n --lines)'{-n,--lines}'[Number of lines]:lines:' \
                    '(-p --priority)'{-p,--priority}'[Priority filter]:priority:(emerg alert crit err warning notice info debug)' \
                    '--all[Aggregate across all sdtab units]' \
                    '--failed[Only failed units (implies --all)]' \
                    '--since[Show entries newer than given time]:time:' \
                    '*:unit:->units'
            else
                _arguments '*:unit:->units'
            fi
            case "$state" in
                units)
                    local -a names
                    names=(${(f)"$(sdtab __names 2>/dev/null)"})
                    _describe -t names 'unit' names
                    ;;
            esac
            ;;
        apply)
            _arguments \
                '--prune[Remove units not in the file]' \
                '--dry-run[Show changes without applying]' \
                '*:file:_files -g "*.toml"'
            ;;
        export)
            _arguments \
                '(-o --output)'{-o,--output}'[Output file path]:file:_files'
            ;;
        list)
            _arguments \
                '--json[Output as JSON]' \
                '--sort[Sort order]:order:(time name)'
            ;;
        init)
            _arguments \
                '--slack-webhook[Slack webhook URL]:url:' \
                '--slack-mention[Slack user/group ID]:id:'
            ;;
        completions)
            _values 'shell' bash zsh fish
            ;;
        add)
            _arguments \
                '--name[Unit name]:name:' \
                '--workdir[Working directory]:dir:_files -/' \
                '--description[Description]:text:' \
                '--env-file[Environment file]:file:_files' \
                '--restart[Restart policy]:policy:(always on-failure no)' \
                '--memory-max[Memory limit]:size:' \
                '--cpu-quota[CPU quota]:percent:' \
                '--io-weight[I/O weight 1-10000]:weight:' \
                '--timeout-stop[Stop timeout]:duration:' \
                '--exec-start-pre[Pre-start command]:cmd:' \
                '--exec-stop-post[Post-stop command]:cmd:' \
                '--log-level-max[Max log level]:level:(emerg alert crit err warning notice info debug)' \
                '--random-delay[Random delay]:duration:' \
                '*--env[Environment variable]:KEY=VALUE:' \
                '--no-notify[Disable failure notification]' \
                '--dry-run[Preview without creating]'
            ;;
    esac
}

_sdtab "$@"
