# unc-lsp in Neovim, Vim and Emacs

First build and install the server. It goes in `~/.cargo/bin`, so make sure that's on your
PATH:

```sh
cargo install --path crates/unc-lsp
```

Then put `editors/vim` on the runtimepath. That gives you the `unc` filetype (see
[the ftdetect rules](../README.md#which-files-are-unc-sources-ftdetect)), the syntax file and
the ftplugin. All three work with or without the LSP.

## Neovim 0.11 and later

Neovim 0.11 has `vim.lsp.config` and `vim.lsp.enable` built in. You don't need a plugin:

```lua
vim.opt.rtp:append('/path/to/unc-64/editors/vim')

vim.lsp.config('unc_lsp', {
  cmd = { 'unc-lsp' },
  filetypes = { 'unc' },
  root_markers = { '.unc', '.git' },
  init_options = { cumulativeCycles = false },  -- true: also "Σ cycles since the last label"
})
vim.lsp.enable('unc_lsp')

-- The cycle counts are inlay hints.
vim.api.nvim_create_autocmd('LspAttach', {
  callback = function(ev)
    if vim.lsp.get_client_by_id(ev.data.client_id).name == 'unc_lsp' then
      vim.lsp.inlay_hint.enable(true, { bufnr = ev.buf })
    end
  end,
})
```

You can also put the table from `vim.lsp.config` in `lsp/unc_lsp.lua` on the runtimepath,
returning it, and call only `vim.lsp.enable('unc_lsp')`.

To switch the cumulative counts on or off while you work, without restarting the server:

```lua
local function unc_cumulative(on)
  for _, c in ipairs(vim.lsp.get_clients({ name = 'unc_lsp' })) do
    c:notify('workspace/didChangeConfiguration', { settings = { unc = { cumulativeCycles = on } } })
  end
end
```

The defaults already map the things you'd expect: `K` hovers, `grn` renames, `gra` lists code
actions, `grr` finds references, `gO` shows the outline, and `<C-]>` goes to the definition.
Completion is `<C-x><C-o>`, or turn on `vim.lsp.completion.enable()` to complete as you type.

## Neovim with nvim-lspconfig (before 0.11)

```lua
local configs = require('lspconfig.configs')
if not configs.unc_lsp then
  configs.unc_lsp = {
    default_config = {
      cmd = { 'unc-lsp' },
      filetypes = { 'unc' },
      root_dir = require('lspconfig.util').root_pattern('.unc', '.git'),
      single_file_support = true,
      init_options = { cumulativeCycles = false },
    },
  }
end
require('lspconfig').unc_lsp.setup({})
```

Neovim 0.10 has inlay hints too: `vim.lsp.inlay_hint.enable(true)`.

## Vim

### vim-lsp

```vim
set runtimepath+=/path/to/unc-64/editors/vim
filetype plugin on
syntax on

if executable('unc-lsp')
  autocmd User lsp_setup call lsp#register_server({
        \ 'name': 'unc-lsp',
        \ 'cmd': {server_info -> ['unc-lsp']},
        \ 'allowlist': ['unc'],
        \ 'initialization_options': {'cumulativeCycles': v:false},
        \ })
endif
let g:lsp_inlay_hints_enabled = 1    " Vim 9.0.0167+ for text properties
```

### coc.nvim

In `:CocConfig`:

```json
{
  "languageserver": {
    "unc": {
      "command": "unc-lsp",
      "filetypes": ["unc"],
      "rootPatterns": [".unc", ".git"],
      "initializationOptions": { "cumulativeCycles": false }
    }
  },
  "inlayHint.enable": true
}
```

### yegappan/lsp (Vim 9)

```vim
call LspAddServer([#{name: 'unc-lsp', filetype: ['unc'], path: 'unc-lsp', args: [],
      \ initializationOptions: #{cumulativeCycles: v:false}}])
call LspOptionsSet(#{showInlayHints: v:true})
```

## Emacs

```elisp
(with-eval-after-load 'eglot (add-to-list 'eglot-server-programs '(asm-mode "unc-lsp")))
```

Then run `M-x eglot` in the `.s` buffer.
