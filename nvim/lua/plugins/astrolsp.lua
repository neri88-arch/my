return {
  "AstroNvim/astrolsp",
  opts = {
    features = {
      codelens = true,
      inlay_hints = false,
      semantic_tokens = true,
    },
    formatting = {
      format_on_save = {
        enabled = false,
      },
      disabled = {},
      timeout_ms = 1000,
    },
    servers = {
      "basedpyright",
      "ruff",
    },
    config = {},
    handlers = {},
    mappings = {
      n = {
        gD = {
          function() vim.lsp.buf.declaration() end,
          desc = "Declaration of current symbol",
          cond = "textDocument/declaration",
        },
      },
    },
    on_attach = function(client, bufnr)
      local map = vim.keymap.set
      local opts = { buffer = bufnr, silent = true }
      map("n", "gd", vim.lsp.buf.definition, opts)
      map("n", "K", vim.lsp.buf.hover, opts)
      map("n", "gi", vim.lsp.buf.implementation, opts)
    end,
  },
}
