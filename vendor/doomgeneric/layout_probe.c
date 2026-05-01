#include <stddef.h>
#include "d_player.h"
#include "d_think.h"
#include "net_defs.h"
#include "d_loop.h"

size_t ROOM_PLAYER_T_SIZEOF = sizeof(player_t);
size_t ROOM_PLAYER_T_MESSAGE_OFFSET = offsetof(player_t, message);

size_t ROOM_THINKER_T_SIZEOF = sizeof(thinker_t);

size_t ROOM_NET_CONNECT_DATA_T_SIZEOF = sizeof(net_connect_data_t);
size_t ROOM_NET_GAMESETTINGS_T_SIZEOF = sizeof(net_gamesettings_t);
size_t ROOM_LOOP_INTERFACE_T_SIZEOF = sizeof(loop_interface_t);
