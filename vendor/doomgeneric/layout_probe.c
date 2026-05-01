#include <stddef.h>
#include "d_player.h"
#include "d_think.h"
#include "net_defs.h"
#include "d_loop.h"
#include "r_local.h"
#include "p_local.h"
#include "p_spec.h"

size_t ROOM_PLAYER_T_SIZEOF = sizeof(player_t);
size_t ROOM_PLAYER_T_MESSAGE_OFFSET = offsetof(player_t, message);

size_t ROOM_THINKER_T_SIZEOF = sizeof(thinker_t);

size_t ROOM_NET_CONNECT_DATA_T_SIZEOF = sizeof(net_connect_data_t);
size_t ROOM_NET_GAMESETTINGS_T_SIZEOF = sizeof(net_gamesettings_t);
size_t ROOM_LOOP_INTERFACE_T_SIZEOF = sizeof(loop_interface_t);

size_t ROOM_SECTOR_T_SIZEOF = sizeof(sector_t);
size_t ROOM_SECTOR_T_LIGHTLEVEL_OFFSET = offsetof(sector_t, lightlevel);
size_t ROOM_SECTOR_T_SPECIAL_OFFSET = offsetof(sector_t, special);
size_t ROOM_SECTOR_T_SPECIALDATA_OFFSET = offsetof(sector_t, specialdata);
size_t ROOM_SECTOR_T_TAG_OFFSET = offsetof(sector_t, tag);

size_t ROOM_FIREFLICKER_T_SIZEOF = sizeof(fireflicker_t);
size_t ROOM_LIGHTFLASH_T_SIZEOF = sizeof(lightflash_t);
size_t ROOM_STROBE_T_SIZEOF = sizeof(strobe_t);
size_t ROOM_GLOW_T_SIZEOF = sizeof(glow_t);
