" Vim filetype plugin for UNC assembly (the UNC 64's assembler, manual chapter 6).

if exists('b:did_ftplugin')
  finish
endif
let b:did_ftplugin = 1

" Comments run from ; to the end of the line.
setlocal comments=:; commentstring=;\ %s
setlocal formatoptions-=t formatoptions+=croql

" @loop is one word, for *, # and Ctrl-].
setlocal iskeyword=@,48-57,_,@-@

" [i, [d, :checkpath and gf know .include "file.s" and the definitions: `name:`, `NAME =`.
setlocal include=\\c^.*\\.include\\s*\"
setlocal define=^\\s*\\ze@\\=\\h\\w*\\s*\\%(:\\\|=[^=]\\)
setlocal suffixesadd=.s

" The manual's layout: labels in column 0, everything else indented by 8 spaces.
setlocal expandtab shiftwidth=8 softtabstop=8

let b:undo_ftplugin = 'setlocal comments< commentstring< formatoptions< iskeyword< include< define< suffixesadd< expandtab< shiftwidth< softtabstop<'
