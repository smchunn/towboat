" macOS-specific Neovim configuration

" Basic settings
set number
set relativenumber
set expandtab
set tabstop=2
set shiftwidth=2
set autoindent
set smartindent

" macOS-specific clipboard
set clipboard=unnamed

" Color scheme
colorscheme slate
set background=dark

" File type detection
filetype plugin indent on
syntax enable

" Key mappings for macOS
nnoremap <Cmd-s> :w<CR>
inoremap <Cmd-s> <Esc>:w<CR>a
vnoremap <Cmd-c> "+y

" Terminal integration
nnoremap <leader>t :terminal<CR>

" macOS specific settings
set mouse=a
set guifont=SF\ Mono:h14

" Plugin management would go here
" call plug#begin('~/.local/share/nvim/plugged')