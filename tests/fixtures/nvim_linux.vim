" Linux-specific Neovim configuration

" Basic settings
set number
set relativenumber
set expandtab
set tabstop=4
set shiftwidth=4
set autoindent
set smartindent

" Linux-specific clipboard
set clipboard=unnamedplus

" Color scheme
colorscheme desert
set background=dark

" File type detection
filetype plugin indent on
syntax enable

" Key mappings for Linux
nnoremap <C-s> :w<CR>
inoremap <C-s> <Esc>:w<CR>a
vnoremap <C-c> "+y

" Terminal integration
nnoremap <leader>t :terminal<CR>

" Plugin management would go here
" call plug#begin('~/.local/share/nvim/plugged')