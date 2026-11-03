" UNC assembly: which files are UNC sources.
"
" *.uncs and *.unc.s are always UNC. A plain *.s is too common to claim (GNU as, other
" 6502 assemblers), so it's UNC only when it says so or lives in a UNC project:
"   - a comment in its first 20 lines names the UNC (`; SNAKE for the UNC 16`), as every
"     source in the manual and the repository does; or
"   - a file or folder called `.unc` is in its folder or a folder above (touch one at the
"     root of your project); or
"   - it's inside the unc-64 repository (crates/unc-asm above it).
" Anything else stays Vim's asm. `let g:unc_all_s = 1` makes every *.s UNC.

autocmd BufNewFile,BufRead *.uncs,*.unc.s set filetype=unc
autocmd BufNewFile,BufRead *.s call s:UncDetect()

function! s:UncDetect() abort
  if &filetype ==# 'unc'
    return
  endif
  if get(g:, 'unc_all_s', 0)
    set filetype=unc
    return
  endif
  for l:line in getline(1, 20)
    if l:line =~# '^\s*;.*\<UNC\>'
      set filetype=unc
      return
    endif
  endfor
  let l:dir = fnameescape(expand('<afile>:p:h'))
  if findfile('.unc', l:dir . ';') !=# '' || finddir('.unc', l:dir . ';') !=# ''
        \ || findfile('crates/unc-asm/Cargo.toml', l:dir . ';') !=# ''
    set filetype=unc
  endif
endfunction
