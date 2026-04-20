function __sdtab_needs_command
    set cmd (commandline -opc)
    if test (count $cmd) -eq 1
        return 0
    end
    return 1
end

function __sdtab_using_command
    set cmd (commandline -opc)
    if test (count $cmd) -ge 2
        if test "$cmd[2]" = "$argv[1]"
            return 0
        end
    end
    return 1
end

function __sdtab_unit_names
    sdtab __names 2>/dev/null
end

# Subcommands
complete -c sdtab -n __sdtab_needs_command -a init -d 'Initialize sdtab'
complete -c sdtab -n __sdtab_needs_command -a add -d 'Add a new timer or service'
complete -c sdtab -n __sdtab_needs_command -a list -d 'List managed units'
complete -c sdtab -n __sdtab_needs_command -a remove -d 'Remove a unit'
complete -c sdtab -n __sdtab_needs_command -a edit -d 'Edit unit files'
complete -c sdtab -n __sdtab_needs_command -a logs -d 'Show logs'
complete -c sdtab -n __sdtab_needs_command -a restart -d 'Restart a unit'
complete -c sdtab -n __sdtab_needs_command -a run -d 'Trigger a unit once manually'
complete -c sdtab -n __sdtab_needs_command -a status -d 'Show detailed status'
complete -c sdtab -n __sdtab_needs_command -a enable -d 'Enable a unit'
complete -c sdtab -n __sdtab_needs_command -a disable -d 'Disable a unit'
complete -c sdtab -n __sdtab_needs_command -a export -d 'Export configuration'
complete -c sdtab -n __sdtab_needs_command -a apply -d 'Apply configuration'
complete -c sdtab -n __sdtab_needs_command -a doctor -d 'Run health checks'
complete -c sdtab -n __sdtab_needs_command -a completions -d 'Generate completion script'

# Dynamic unit name completion for name-taking subcommands
for cmd in logs status edit remove enable disable restart run
    complete -c sdtab -n "__sdtab_using_command $cmd" -f -a '(__sdtab_unit_names)'
end

# logs flags
complete -c sdtab -n '__sdtab_using_command logs' -s f -l follow -d 'Follow log output'
complete -c sdtab -n '__sdtab_using_command logs' -s n -l lines -d 'Number of lines' -x
complete -c sdtab -n '__sdtab_using_command logs' -s p -l priority -d 'Priority filter' -xa 'emerg alert crit err warning notice info debug'
complete -c sdtab -n '__sdtab_using_command logs' -l all -d 'Aggregate across all sdtab units'
complete -c sdtab -n '__sdtab_using_command logs' -l failed -d 'Only failed units (implies --all)'
complete -c sdtab -n '__sdtab_using_command logs' -l since -d 'Show entries newer than given time' -x

# completions
complete -c sdtab -n '__sdtab_using_command completions' -f -a 'bash zsh fish'

# apply
complete -c sdtab -n '__sdtab_using_command apply' -l prune -d 'Remove units not in file'
complete -c sdtab -n '__sdtab_using_command apply' -l dry-run -d 'Show changes without applying'

# export
complete -c sdtab -n '__sdtab_using_command export' -s o -l output -d 'Output file' -r

# list
complete -c sdtab -n '__sdtab_using_command list' -l json -d 'Output as JSON'
complete -c sdtab -n '__sdtab_using_command list' -l sort -d 'Sort order' -xa 'time name'
