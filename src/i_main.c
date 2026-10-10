// Emacs style mode select   -*- C++ -*-
//-----------------------------------------------------------------------------
//
// $Id:$
//
// Copyright (C) 1993-1996 by id Software, Inc.
// Modified for CoolDoom and wDoom in 2026.
// id Software later released the DOOM engine source under GNU GPL v2.
// This modified file is distributed under GPL v2; see LICENSE and NOTICE.md.
// See Git history for modification details.
//
// This source is available for distribution and/or modification
// only under the terms of the DOOM Source Code License as
// published by id Software. All rights reserved.
//
// The source is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// FITNESS FOR A PARTICULAR PURPOSE. See the DOOM Source Code License
// for more details.
//
// $Log:$
//
// DESCRIPTION:
//	Main program, simply calls D_DoomMain high level loop.
//
//-----------------------------------------------------------------------------





#include "doomdef.h"

#include "m_argv.h"
#include "d_main.h"
#include <stdio.h>
int
main
( int		argc,
  char**	argv )
{
    printf("wDoom\n");
    printf("Copyright (C) 1993-1996 id Software, Inc.; 2026 brubid and Ethan Herman\n");
    printf("Licensed under GNU GPL v2. ABSOLUTELY NO WARRANTY.\n");
    printf("See LICENSE and NOTICE.md for license terms, source, and attribution.\n\n");
    myargc = argc;
    myargv = argv;

    D_DoomMain ();

    return 0;
}
