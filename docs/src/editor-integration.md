# Editor Integration

diesel-guard ships with a built-in LSP (Language Server Protocol) server. Once configured, your editor will show migration violations as inline diagnostics while you edit `.sql` files — no manual `diesel-guard check` needed.

Start the server with:

```
diesel-guard lsp
```

Editors spawn this automatically. The server communicates over stdin/stdout using JSON-RPC.

## VS Code

Install the **diesel-guard** extension from the VS Code Marketplace, or build it from source:

```bash
cd editors/vscode
npm install && npm run compile
npx vsce package
code --install-extension diesel-guard-*.vsix
```

If `diesel-guard` is not on your `PATH`, set the binary path in VS Code settings:

```json
{
  "diesel-guard.binaryPath": "/path/to/diesel-guard"
}
```

## Helix

Add to `~/.config/helix/languages.toml`:

```toml
[[language]]
name = "sql"
language-servers = ["diesel-guard"]

[language-server.diesel-guard]
command = "diesel-guard"
args = ["lsp"]
```

## Zed

Add to your Zed `settings.json`:

```json
{
  "lsp": {
    "diesel-guard": {
      "binary": {
        "path": "diesel-guard",
        "args": ["lsp"]
      }
    }
  },
  "languages": {
    "SQL": {
      "language_servers": ["diesel-guard"]
    }
  }
}
```

## Neovim

Requires the [`nvim-lspconfig`](https://github.com/neovim/nvim-lspconfig) plugin. Add to your `~/.config/nvim/init.lua`:

```lua
local lspconfig = require('lspconfig')
local configs = require('lspconfig.configs')

if not configs.diesel_guard then
  configs.diesel_guard = {
    default_config = {
      cmd = { 'diesel-guard', 'lsp' },
      filetypes = { 'sql' },
      root_dir = lspconfig.util.root_pattern('diesel-guard.toml', '.git'),
    },
  }
end

lspconfig.diesel_guard.setup {}
```

## Emacs

Requires Emacs 29+ (built-in `eglot`). Add to `~/.emacs.d/init.el`:

```elisp
(with-eval-after-load 'eglot
  (add-to-list 'eglot-server-programs
               '(sql-mode . ("diesel-guard" "lsp"))))

(add-hook 'sql-mode-hook 'eglot-ensure)
```

## Sublime Text

Requires the [LSP](https://packagecontrol.io/packages/LSP) package. Open **Preferences → Package Settings → LSP → Settings** and add:

```json
{
  "clients": {
    "diesel-guard": {
      "enabled": true,
      "command": ["diesel-guard", "lsp"],
      "selector": "source.sql"
    }
  }
}
```

## Notes

- The LSP server loads `diesel-guard.toml` from the directory it is launched in. Editors typically use the workspace root as the working directory, so configuration is picked up automatically.
- Only `.sql` files receive diagnostics.
- Diagnostics are published on every keystroke (using in-memory content) and refreshed with full migration context (including `metadata.toml` / `-- no-transaction`) on save.
