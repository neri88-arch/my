return {
  {
    "CRAG666/code_runner.nvim",
    config = function()
      require("code_runner").setup({
        filetype = {
          python = "python3 %",
          rust = "cd $dir && rustc % && ./$fileNoExt",
          sh = "bash %",
        },
      })
    end,
  },
}
