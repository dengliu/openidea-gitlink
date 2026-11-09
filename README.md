# GitLink for OpenIDEA

An [OpenIDEA](https://github.com/l0ng-ai/openidea) plugin (`dengliu.gitlink`) that opens or copies a remote link (or Markdown link) to a file, line selection, directory, or commit in your browser.

Ported from [ben-gibson/GitLink](https://github.com/ben-gibson/GitLink) for JetBrains IDEs.

## Supported Hosting Platforms

Auto-detects the hosting provider from your Git remote URL, with support for:

- **GitHub** (`github.com` and GitHub Enterprise)
- **GitLab** (`gitlab.com` and self-hosted instances)
- **Bitbucket Cloud** (`bitbucket.org`)
- **Bitbucket Server / Data Center** (`stash` / `bitbucket` domains, including `/scm/` URL normalization)
- **Codeberg** (`codeberg.org`)
- **Forgejo**
- **Gitea**
- **Gitee** (`gitee.com`)
- **Gogs**
- **Azure DevOps** (`dev.azure.com`, `ssh.dev.azure.com`, `*.visualstudio.com`)
- **sourcehut** (`git.sr.ht`)
- **Gerrit** (Gitiles URLs)
- **Custom URL templates** (`{remote:url}`, `{remote:url:protocol}`, `{remote:url:host}`, `{remote:url:path}`, `{remote:url:path:N}`, `{object}`, `{branch}`, `{commit}`, `{commit:short}`, `{file:path}`, `{file:name}`, `{line-block:start}...{line:start}...{line:end}...{line-block:end}`)

## Commands & Menus

Contributed to the **Editor context menu**, **Project View context menu**, **Editor Tab context menu**, **Tools** menu, and **Search Everywhere / Find Action**:

- `GitLink: Open in Browser` (`dengliu.gitlink.open`)
- `GitLink: Copy Link` (`dengliu.gitlink.copy`)
- `GitLink: Copy Markdown Link` (`dengliu.gitlink.copy-markdown`)
- `GitLink: Open Commit in Browser` (`dengliu.gitlink.open-commit`)
- `GitLink: Copy Commit Link` (`dengliu.gitlink.copy-commit`)
- `GitLink: Copy Commit Markdown Link` (`dengliu.gitlink.copy-commit-markdown`)

By default, when `HEAD` has been pushed to its upstream remote branch, GitLink generates a permanent link using the commit SHA. When `HEAD` has unpushed commits (or no upstream), it falls back to the branch name so the link does not 404.

## Capabilities

| Capability | Why it is needed |
| --- | --- |
| `project.read` | Checks whether a selected path inside the project is a file or directory. |
| `editor.read` | Reads the active editor's file path and selected line range (`#L<start>-L<end>`). |
| `vcs.read` | Reads the repository's configured remotes (`vcs.remotes`) and branch/commit/upstream status (`vcs.status`). |
| `clipboard` | Copies generated URLs and Markdown links to the system clipboard (`ui.copy-to-clipboard`). |
| `fs.read:~/.ssh/config` *(optional)* | Resolves SSH `Host` aliases (e.g. `git@github-personal:owner/repo.git` → `github.com`) from `~/.ssh/config` when the host has not already resolved them. |

## Building & Testing

```sh
cargo test
openidea-plugin build
openidea-plugin check
```

## License

[MIT](LICENSE) — ported from [ben-gibson/GitLink](https://github.com/ben-gibson/GitLink) (Copyright (c) 2022 Ben Gibson).
