return {
  {
    "stevearc/conform.nvim",
    event = { "BufWritePre" },
    cmd = { "ConformInfo" },
    opts = function(_, opts)
      opts.formatters_by_ft = {
        python = { "ruff_format", "ruff_organize_imports" },
        rust = { "rustfmt" },
        bash = { "shfmt" },
        sh = { "shfmt" },
      }
      opts.format_on_save = function(bufnr)
        if vim.g.disable_autoformat or vim.b[bufnr].disable_autoformat then
          return
        end
        return { timeout_ms = 500, lsp_fallback = true }
      end
    end,
    init = function()
      vim.o.formatexpr = "v:lua.require'conform'.formatexpr()"
    end,
  },
}
