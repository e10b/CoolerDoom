#include <stddef.h>
#include "d_player.h"
#include "d_think.h"
#include "net_defs.h"
#include "d_loop.h"
#include "r_local.h"
#include "p_local.h"
#include "p_spec.h"
#include "st_lib.h"

/*
 * Anchor function referenced from Rust (d_player.rs) to force the
 * linker to include this object file in all build targets (including
 * test binaries where dead-code elimination would otherwise discard it).
 */
void __room_layout_probe_anchor(void) {}

size_t ROOM_SECTOR_T_SIZEOF = sizeof(sector_t);
size_t ROOM_SECTOR_T_LIGHTLEVEL_OFFSET = offsetof(sector_t, lightlevel);
size_t ROOM_SECTOR_T_SPECIAL_OFFSET = offsetof(sector_t, special);
size_t ROOM_SECTOR_T_SPECIALDATA_OFFSET = offsetof(sector_t, specialdata);
size_t ROOM_SECTOR_T_TAG_OFFSET = offsetof(sector_t, tag);

size_t ROOM_FIREFLICKER_T_SIZEOF = sizeof(fireflicker_t);
size_t ROOM_LIGHTFLASH_T_SIZEOF = sizeof(lightflash_t);
size_t ROOM_STROBE_T_SIZEOF = sizeof(strobe_t);
size_t ROOM_GLOW_T_SIZEOF = sizeof(glow_t);

size_t ROOM_ST_NUMBER_T_SIZEOF = sizeof(st_number_t);
size_t ROOM_ST_PERCENT_T_SIZEOF = sizeof(st_percent_t);
size_t ROOM_ST_MULTICON_T_SIZEOF = sizeof(st_multicon_t);
size_t ROOM_ST_BINICON_T_SIZEOF = sizeof(st_binicon_t);
