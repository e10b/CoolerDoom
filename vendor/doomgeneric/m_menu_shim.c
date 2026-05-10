// Shim for m_menu to set player message without needing full player_t layout in Rust.
#include "g_game.h"
#include "doomstat.h"

void M_Menu_SetPlayerMessage(const char *msg)
{
    players[consoleplayer].message = (char *)msg;
}
