_markdown__viewer() {
    local i cur prev opts cmd
    COMPREPLY=()
    if [[ "${BASH_VERSINFO[0]}" -ge 4 ]]; then
        cur="$2"
    else
        cur="${COMP_WORDS[COMP_CWORD]}"
    fi
    prev="$3"
    cmd=""
    opts=""

    for i in "${COMP_WORDS[@]:0:COMP_CWORD}"
    do
        case "${cmd},${i}" in
            ",$1")
                cmd="markdown__viewer"
                ;;
            *)
                ;;
        esac
    done

    case "${cmd}" in
        markdown__viewer)
            opts="-w -o -p -h -V --watch --output --pdf --clipboard --goto --if-running --check-links --online --port --no-open --browser --app --window --tab --exit-when-closed --theme --style --css --number-headings --line-numbers --front-matter --obsidian --no-obsidian --editor --generate-completions --generate-man --install-finder --uninstall-finder --help --version"
            if [[ ${cur} == -* || ${COMP_CWORD} -eq 1 ]] ; then
                COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
                return 0
            fi
            case "${prev}" in
                --output)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -o)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --pdf)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --goto)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --port)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                -p)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --browser)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --theme)
                    COMPREPLY=($(compgen -W "auto light dark" -- "${cur}"))
                    return 0
                    ;;
                --style)
                    COMPREPLY=($(compgen -W "github wide book compact" -- "${cur}"))
                    return 0
                    ;;
                --css)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --front-matter)
                    COMPREPLY=($(compgen -W "table hide" -- "${cur}"))
                    return 0
                    ;;
                --editor)
                    COMPREPLY=($(compgen -f "${cur}"))
                    return 0
                    ;;
                --generate-completions)
                    COMPREPLY=($(compgen -W "bash elvish fish powershell zsh" -- "${cur}"))
                    return 0
                    ;;
                *)
                    COMPREPLY=()
                    ;;
            esac
            COMPREPLY=( $(compgen -W "${opts}" -- "${cur}") )
            return 0
            ;;
    esac
}

if [[ "${BASH_VERSINFO[0]}" -eq 4 && "${BASH_VERSINFO[1]}" -ge 4 || "${BASH_VERSINFO[0]}" -gt 4 ]]; then
    complete -F _markdown__viewer -o nosort -o bashdefault -o default markdown-viewer
else
    complete -F _markdown__viewer -o bashdefault -o default markdown-viewer
fi
