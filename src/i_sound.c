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
//	System interface for sound.
//
//-----------------------------------------------------------------------------


#include <SDL3/SDL.h>
#include <stdio.h>
#include <stdlib.h>
#include <stdarg.h>
#include <stdint.h>
#include <string.h>

#include <math.h>


#include <sys/types.h>

#ifndef LINUX
//#include <sys/filio.h>
#endif

#include <fcntl.h>
#ifdef _WIN32
#include <io.h>
#else
#include <unistd.h>
#endif
#include <SDL3/SDL_audio.h>
//#include <ioctl.h>
// Linux voxware output.
//#include <linux/soundcard.h>

// Timer stuff. Experimental.
#include <time.h>
#include <SDL3/SDL_timer.h>
#include <signal.h>

#include "z_zone.h"

#include "i_system.h"
#include "i_sound.h"
#include "m_argv.h"
#include "m_misc.h"
#include "w_wad.h"

#include "doomdef.h"
SDL_AudioStream* mainAudio;
// UNIX hack, to be removed.
#ifdef SNDSERV
// Separate sound server process.
FILE*	sndserver=0;
char*	sndserver_filename = "./sndserver ";
#elif SNDINTR

// Update all 30 millisecs, approx. 30fps synchronized.
// Linux resolution is allegedly 10 millisecs,
//  scale is microseconds.
#define SOUND_INTERVAL     500

// Get the interrupt. Set duration in millisecs.
int I_SoundSetTimer( int duration_of_tick );
void I_SoundDelTimer( void );
#else
// None?
#endif


// A quick hack to establish a protocol between
// synchronous mix buffer updates and asynchronous
// audio writes. Probably redundant with gametic.
static int flag = 0;

// The number of internal mixing channels,
//  the samples calculated for each mixing step,
//  the size of the 16bit, 2 hardware channel (stereo)
//  mixing buffer, and the samplerate of the raw data.


// Needed for calling the actual sound output.
#define SAMPLECOUNT		315
#define NUM_CHANNELS		8
// It is 2 for 16bit, and 2 for two channels.
#define BUFMUL                  4
#define MIXBUFFERSIZE		(SAMPLECOUNT*BUFMUL)

#define SAMPLERATE		11025    	// Hz
#define SAMPLESIZE		2   	// 16bit

// The actual lengths of all sound effects.
int 		lengths[NUMSFX];

// The actual output device.
int	audio_fd;

// The global mixing buffer.
// Basically, samples from all active internal channels
//  are modifed and added, and stored in the buffer
//  that is submitted to the audio device.
signed short	mixbuffer[MIXBUFFERSIZE];


// The channel step amount...
unsigned int	channelstep[NUM_CHANNELS];
// ... and a 0.16 bit remainder of last step.
unsigned int	channelstepremainder[NUM_CHANNELS];


// The channel data pointers, start and end.
unsigned char*	channels[NUM_CHANNELS];
unsigned char*	channelsend[NUM_CHANNELS];


// Time/gametic that the channel started playing,
//  used to determine oldest, which automatically
//  has lowest priority.
// In case number of active sounds exceeds
//  available channels.
int		channelstart[NUM_CHANNELS];

// The sound in channel handles,
//  determined on registration,
//  might be used to unregister/stop/modify,
//  currently unused.
int 		channelhandles[NUM_CHANNELS];

// SFX id of the playing sound effect.
// Used to catch duplicates (like chainsaw).
int		channelids[NUM_CHANNELS];

// Pitch to stepping lookup, unused.
int		steptable[256];

// Volume lookups.
int		vol_lookup[128*256];

// Hardware left and right channel volume lookup.
int*		channelleftvol_lookup[NUM_CHANNELS];
int*		channelrightvol_lookup[NUM_CHANNELS];



int currentAudioTime, oldAudioTime;

// Lightweight MUS synthesizer. DOOM stores music as sequenced MUS events,
// rather than sampled audio; keeping the sequencer in the existing SDL mix
// path makes it work identically in native and Emscripten builds.
#define MUSIC_VOICES 48
#define MUS_TICK_RATE 140

typedef struct
{
  int active;
  int channel;
  int note;
  int velocity;
  int releasing;
  int percussion;
  double phase;
  double step;
  double level;
} music_voice_t;

static music_voice_t music_voices[MUSIC_VOICES];
static const unsigned char* music_score_start;
static const unsigned char* music_score_end;
static const unsigned char* music_position;
static int music_active;
static int music_paused;
static int music_looping;
static int music_samples_until_event;
static int music_channel_volume[16];
static int music_channel_program[16];
static int music_channel_velocity[16];
static uint32_t music_noise = 1;

static int MusicRead16(const unsigned char* p)
{
  return p[0] | (p[1] << 8);
}

static void MusicResetVoices(void)
{
  int i;
  memset(music_voices, 0, sizeof(music_voices));
  for (i = 0; i < 16; ++i)
  {
    music_channel_volume[i] = 100;
    music_channel_program[i] = 0;
    music_channel_velocity[i] = 64;
  }
}

static void MusicReleaseChannel(int channel)
{
  int i;
  for (i = 0; i < MUSIC_VOICES; ++i)
    if (music_voices[i].active && music_voices[i].channel == channel)
      music_voices[i].releasing = 1;
}

static void MusicReleaseNote(int channel, int note)
{
  int i;
  for (i = 0; i < MUSIC_VOICES; ++i)
    if (music_voices[i].active && music_voices[i].channel == channel &&
        music_voices[i].note == note)
      music_voices[i].releasing = 1;
}

static void MusicStartNote(int channel, int note, int velocity)
{
  int i;
  int slot = 0;
  double frequency;
  double quietest = 2.0;

  for (i = 0; i < MUSIC_VOICES; ++i)
  {
    if (!music_voices[i].active)
    {
      slot = i;
      break;
    }
    if (music_voices[i].level < quietest)
    {
      quietest = music_voices[i].level;
      slot = i;
    }
  }

  frequency = 440.0 * pow(2.0, (note - 69) / 12.0);
  music_voices[slot].active = 1;
  music_voices[slot].channel = channel;
  music_voices[slot].note = note;
  music_voices[slot].velocity = velocity;
  music_voices[slot].releasing = 0;
  music_voices[slot].percussion = channel == 15;
  music_voices[slot].phase = 0.0;
  music_voices[slot].step = frequency / SAMPLERATE;
  music_voices[slot].level = 1.0;
}

static void MusicRestart(void)
{
  music_position = music_score_start;
  music_samples_until_event = 0;
  MusicResetVoices();
}

static int MusicReadByte(void)
{
  if (music_position >= music_score_end)
    return -1;
  return *music_position++;
}

static void MusicProcessEvents(void)
{
  while (music_active && music_samples_until_event <= 0)
  {
    int descriptor = MusicReadByte();
    int channel;
    int type;
    int last;
    int value;

    if (descriptor < 0)
    {
      if (music_looping)
        MusicRestart();
      else
        music_active = 0;
      return;
    }
    else
    {
      channel = descriptor & 15;
      type = (descriptor >> 4) & 7;
      last = descriptor & 128;

      switch (type)
      {
        case 0:
          value = MusicReadByte();
          if (value >= 0) MusicReleaseNote(channel, value & 127);
          break;
        case 1:
          value = MusicReadByte();
          if (value >= 0)
          {
            int note = value & 127;
            if (value & 128)
            {
              value = MusicReadByte();
              if (value >= 0) music_channel_velocity[channel] = value & 127;
            }
            MusicStartNote(channel, note, music_channel_velocity[channel]);
          }
          break;
        case 2:
          // Pitch wheel is deliberately ignored by this compact synth.
          MusicReadByte();
          break;
        case 3:
          value = MusicReadByte();
          if (value == 10 || value == 11) MusicReleaseChannel(channel);
          break;
        case 4:
        {
          int controller = MusicReadByte();
          value = MusicReadByte();
          if (controller == 0 && value >= 0)
            music_channel_program[channel] = value & 127;
          else if (controller == 3 && value >= 0)
            music_channel_volume[channel] = value & 127;
          break;
        }
        case 6:
          if (music_looping)
          {
            MusicRestart();
            return;
          }
          music_active = 0;
          return;
        default:
          music_active = 0;
          return;
      }

      if (last)
      {
        int delay = 0;
        do
        {
          value = MusicReadByte();
          if (value < 0)
          {
            music_active = 0;
            return;
          }
          delay = delay * 128 + (value & 127);
        } while (value & 128);
        music_samples_until_event += (delay * SAMPLERATE) / MUS_TICK_RATE;
      }
    }

    if (type == 6)
    {
      if (music_looping)
        MusicRestart();
      else
        music_active = 0;
      return;
    }
  }
}

static int MusicRenderSample(void)
{
  int i;
  double mixed = 0.0;

  if (!music_active || music_paused || !music_score_start)
    return 0;

  MusicProcessEvents();
  if (music_samples_until_event > 0)
    --music_samples_until_event;

  for (i = 0; i < MUSIC_VOICES; ++i)
  {
    music_voice_t* voice = &music_voices[i];
    double wave;
    double gain;
    int program;
    if (!voice->active) continue;

    if (voice->percussion)
    {
      music_noise = music_noise * 1664525u + 1013904223u;
      wave = ((music_noise >> 16) / 32768.0) - 1.0;
      voice->level *= 0.997;
    }
    else
    {
      program = music_channel_program[voice->channel];
      if ((program & 3) == 0)
        wave = voice->phase < 0.5 ? 1.0 : -1.0;
      else if ((program & 3) == 1)
        wave = 1.0 - 4.0 * fabs(voice->phase - 0.5);
      else if ((program & 3) == 2)
        wave = 2.0 * voice->phase - 1.0;
      else
        wave = sin(voice->phase * 6.283185307179586);
      voice->phase += voice->step;
      voice->phase -= (int)voice->phase;
    }

    if (voice->releasing)
      voice->level *= 0.996;
    if (voice->level < 0.002)
    {
      voice->active = 0;
      continue;
    }

    gain = (voice->velocity / 127.0) *
           (music_channel_volume[voice->channel] / 127.0) *
           (snd_MusicVolume / 15.0);
    mixed += wave * voice->level * gain * 1500.0;
  }
  return (int)mixed;
}
//
// Safe ioctl, convenience.
//
void
myioctl
( int	fd,
  int	command,
  int*	arg )
{
    /*
    int		rc;
    extern int	errno;

    rc = ioctl(fd, command, arg);
    if (rc < 0)
    {
	fprintf(stderr, "ioctl(dsp,%d,arg) failed\n", command);
	fprintf(stderr, "errno=%d\n", errno);
	exit(-1);
    }*/
}





//
// This function loads the sound data from the WAD lump,
//  for single sound.
//
void*
getsfx
( char*         sfxname,
  int*          len )
{
    unsigned char*      sfx;
    unsigned char*      paddedsfx;
    int                 i;
    int                 size;
    int                 paddedsize;
    char                name[20];
    int                 sfxlump;


    // Get the sound data from the WAD, allocate lump
    //  in zone memory.
    sprintf(name, "ds%s", sfxname);

    // Now, there is a severe problem with the
    //  sound handling, in it is not (yet/anymore)
    //  gamemode aware. That means, sounds from
    //  DOOM II will be requested even with DOOM
    //  shareware.
    // The sound list is wired into sounds.c,
    //  which sets the external variable.
    // I do not do runtime patches to that
    //  variable. Instead, we will use a
    //  default sound for replacement.
    if (W_CheckNumForName(name) == -1)
    {

        return NULL;
//      sfxlump = W_GetNumForName("dspistol");
    }
    else
    {
      sfxlump = W_GetNumForName(name);
    }
    size = W_LumpLength( sfxlump );

    // Debug.
    // fprintf( stderr, "." );
    //fprintf( stderr, " -loading  %s (lump %d, %d bytes)\n",
    //	     sfxname, sfxlump, size );
    //fflush( stderr );

    sfx = (unsigned char*)W_CacheLumpNum( sfxlump, PU_STATIC );

    // Pads the sound effect out to the mixing buffer size.
    // The original realloc would interfere with zone memory.
    paddedsize = ((size-8 + (SAMPLECOUNT-1)) / SAMPLECOUNT) * SAMPLECOUNT;

    // Allocate from zone memory.
    paddedsfx = (unsigned char*)Z_Malloc( paddedsize+8, PU_STATIC, 0 );
    // ddt: (unsigned char *) realloc(sfx, paddedsize+8);
    // This should interfere with zone memory handling,
    //  which does not kick in in the soundserver.

    // Now copy and pad.
    memcpy(  paddedsfx, sfx, size );
    for (i=size ; i<paddedsize+8 ; i++)
        paddedsfx[i] = 128;

    // Remove the cached lump.
    Z_Free( sfx );

    // Preserve padded length.
    *len = paddedsize;

    // Return allocated padded data.
    return (void *) (paddedsfx + 8);
}





//
// This function adds a sound to the
//  list of currently active sounds,
//  which is maintained as a given number
//  (eight, usually) of internal channels.
// Returns a handle.
//
int
addsfx
( int		sfxid,
  int		volume,
  int		step,
  int		seperation )
{
    static unsigned short	handlenums = 0;

    int		i;
    int		rc = -1;

    int		oldest = gametic;
    int		oldestnum = 0;
    int		slot;

    int		rightvol;
    int		leftvol;

    // Chainsaw troubles.
    // Play these sound effects only one at a time.
    if ( sfxid == sfx_sawup
	 || sfxid == sfx_sawidl
	 || sfxid == sfx_sawful
	 || sfxid == sfx_sawhit
	 || sfxid == sfx_stnmov
	 || sfxid == sfx_pistol	 )
    {
	// Loop all channels, check.
	for (i=0 ; i<NUM_CHANNELS ; i++)
	{
	    // Active, and using the same SFX?
	    if ( (channels[i])
		 && (channelids[i] == sfxid) )
	    {
		// Reset.
		channels[i] = 0;
		// We are sure that iff,
		//  there will only be one.
		break;
	    }
	}
    }

    // Loop all channels to find oldest SFX.
    for (i=0; (i<NUM_CHANNELS) && (channels[i]); i++)
    {
	if (channelstart[i] < oldest)
	{
	    oldestnum = i;
	    oldest = channelstart[i];
	}
    }

    // Tales from the cryptic.
    // If we found a channel, fine.
    // If not, we simply overwrite the first one, 0.
    // Probably only happens at startup.
    if (i == NUM_CHANNELS)
	slot = oldestnum;
    else
	slot = i;

    // Okay, in the less recent channel,
    //  we will handle the new SFX.
    // Set pointer to raw data.
    channels[slot] = (unsigned char *) S_sfx[sfxid].data;
    // Set pointer to end of raw data.
    channelsend[slot] = channels[slot] + lengths[sfxid];

    // Reset current handle number, limited to 0..100.
    if (!handlenums)
	handlenums = 100;

    // Assign current handle number.
    // Preserved so sounds could be stopped (unused).
    channelhandles[slot] = rc = handlenums++;

    // Set stepping???
    // Kinda getting the impression this is never used.
    channelstep[slot] = step;
    // ???
    channelstepremainder[slot] = 0;
    // Should be gametic, I presume.
    channelstart[slot] = gametic;

    // Separation, that is, orientation/stereo.
    //  range is: 1 - 256
    seperation += 1;

    // Per left/right channel.
    //  x^2 seperation,
    //  adjust volume properly.
    leftvol =
	volume - ((volume*seperation*seperation) >> 16); ///(256*256);
    seperation = seperation - 257;
    rightvol =
	volume - ((volume*seperation*seperation) >> 16);

    // Sanity check, clamp volume.
    if (rightvol < 0 || rightvol > 127)
	I_Error("rightvol out of bounds");

    if (leftvol < 0 || leftvol > 127)
	I_Error("leftvol out of bounds");

    // Get the proper lookup table piece
    //  for this volume level???
    channelleftvol_lookup[slot] = &vol_lookup[leftvol*256];
    channelrightvol_lookup[slot] = &vol_lookup[rightvol*256];

    // Preserve sound SFX id,
    //  e.g. for avoiding duplicates of chainsaw.
    channelids[slot] = sfxid;

    // You tell me.
    return rc;
}





//
// SFX API
// Note: this was called by S_Init.
// However, whatever they did in the
// old DPMS based DOS version, this
// were simply dummies in the Linux
// version.
// See soundserver initdata().
//
void I_SetChannels()
{
  // Init internal lookups (raw data, mixing buffer, channels).
  // This function sets up internal lookups used during
  //  the mixing process.
  int		i;
  int		j;

  int*	steptablemid = steptable + 128;

  // Okay, reset internal mixing channels to zero.
  for (i=0; i<NUM_CHANNELS; i++)
  {
    channels[i] = 0;
  }

  // This table provides step widths for pitch parameters.
  // I fail to see that this is currently used.
  for (i=-128 ; i<128 ; i++)
    steptablemid[i] = (int)(pow(2.0, (i/64.0))*65536.0);


  // Generates volume lookup tables
  //  which also turn the unsigned samples
  //  into signed samples.
  for (i=0 ; i<128 ; i++)
    for (j=0 ; j<256 ; j++)
      vol_lookup[i*256+j] = (i*(j-128)*256)/127;
}


void I_SetSfxVolume(int volume)
{
  // Identical to DOS.
  // Basically, this should propagate
  //  the menu/config file setting
  //  to the state variable used in
  //  the mixing.
  snd_SfxVolume = volume;
}

// MUSIC API - dummy. Some code from DOS version.
void I_SetMusicVolume(int volume)
{
  // Internal state variable.
  snd_MusicVolume = volume;
  // Now set volume on output device.
  // Whatever( snd_MusciVolume );
}


//
// Retrieve the raw data lump index
//  for a given SFX name.
//
int I_GetSfxLumpNum(sfxinfo_t* sfx)
{
    char namebuf[9];
    sprintf(namebuf, "ds%s", sfx->name);
    return W_GetNumForName(namebuf);
}

//
// Starting a sound means adding it
//  to the current list of active sounds
//  in the internal channels.
// As the SFX info struct contains
//  e.g. a pointer to the raw data,
//  it is ignored.
// As our sound handling does not handle
//  priority, it is ignored.
// Pitching (that is, increased speed of playback)
//  is set, but currently not used by mixing.
//
int
I_StartSound
( int		id,
  int		vol,
  int		sep,
  int		pitch,
  int		priority )
{

  // UNUSED
  priority = 0;
    id = addsfx( id, vol, steptable[pitch], sep );
#ifdef SNDSERV
    if (sndserver)
    {
	fprintf(sndserver, "p%2.2x%2.2x%2.2x%2.2x\n", id, pitch, vol, sep);
	fflush(sndserver);
    }
    // warning: control reaches end of non-void function.
    return id;
#else
    // Debug.
    //fprintf( stderr, "starting sound %d", id );

    // Returns a handle (not used).
  //  id = addsfx( id, vol, steptable[pitch], sep );

    // fprintf( stderr, "/handle is %d\n", id );

    return id;
#endif
}



void I_StopSound (int handle)
{
  // You need the handle returned by StartSound.
  // Would be looping all channels,
  //  tracking down the handle,
  //  an setting the channel to zero.

  // UNUSED.
  handle = 0;
}


int I_SoundIsPlaying(int handle)
{
    // Ouch.
    return gametic < handle;
}




//
// This function loops all active (internal) sound
//  channels, retrieves a given number of samples
//  from the raw sound data, modifies it according
//  to the current (internal) channel parameters,
//  mixes the per channel samples into the global
//  mixbuffer, clamping it to the allowed range,
//  and sets up everything for transferring the
//  contents of the mixbuffer to the (two)
//  hardware channels (left and right, that is).
//
// This function currently supports only 16bit.
//
void I_UpdateSound( void )
{

#ifdef SNDINTR
  // Debug. Count buffer misses with interrupt.
  static int misses = 0;
#endif


  // Mix current sound data.
  // Data, from raw sound, for right and left.
  register unsigned int	sample;
  register int		dl;
  register int		dr;

  // Pointers in global mixbuffer, left, right, end.
  signed short*		leftout;
  signed short*		rightout;
  signed short*		leftend;
  // Step in mixbuffer, left and right, thus two.
  int				step;

  // Mixing channel index.
  int				chan;

    // Left and right channel
    //  are in global mixbuffer, alternating.
    leftout = mixbuffer;
    rightout = mixbuffer+1;
    step = 2;

    // Determine end, for left channel only
    //  (right channel is implicit).
    leftend = mixbuffer + SAMPLECOUNT*step;

    // Mix sounds into the mixing buffer.
    // Loop over step*SAMPLECOUNT,
    //  that is 512 values for two channels.
    while (leftout != leftend)
    {
	// Reset left/right value.
	dl = 0;
	dr = 0;

	// Love thy L2 chache - made this a loop.
	// Now more channels could be set at compile time
	//  as well. Thus loop those  channels.
	for ( chan = 0; chan < NUM_CHANNELS; chan++ )
	{
	    // Check channel, if active.
	    if (channels[ chan ])
	    {
		// Get the raw data from the channel.
		sample = *channels[ chan ];
		// Add left and right part
		//  for this channel (sound)
		//  to the current data.
		// Adjust volume accordingly.
		dl += channelleftvol_lookup[ chan ][sample];
		dr += channelrightvol_lookup[ chan ][sample];
		// Increment index ???
		channelstepremainder[ chan ] += channelstep[ chan ];
		// MSB is next sample???
		channels[ chan ] += channelstepremainder[ chan ] >> 16;
		// Limit to LSB???
		channelstepremainder[ chan ] &= 65536-1;

		// Check whether we are done.
		if (channels[ chan ] >= channelsend[ chan ])
		    channels[ chan ] = 0;
	    }
	}

	// Music shares the same SDL stream as the sound effects.
	sample = MusicRenderSample();
	dl += (int)sample;
	dr += (int)sample;

	// Clamp to range. Left hardware channel.
	// Has been char instead of short.
	// if (dl > 127) *leftout = 127;
	// else if (dl < -128) *leftout = -128;
	// else *leftout = dl;

	if (dl > 0x7fff)
	    *leftout = 0x7fff;
	else if (dl < -0x8000)
	    *leftout = -0x8000;
	else
	    *leftout = dl;

	// Same for right hardware channel.
	if (dr > 0x7fff)
	    *rightout = 0x7fff;
	else if (dr < -0x8000)
	    *rightout = -0x8000;
	else
	    *rightout = dr;

	// Increment current pointers in mixbuffer.
	leftout += step;
	rightout += step;
    }

    // Debug check.
    /*
    if ( flag )
    {
      misses += flag;
      flag = 0;
    }
*/
    if ( misses > 10 )
    {
      fprintf( stderr, "I_SoundUpdate: missed 10 buffer writes\n");
      misses = 0;
    }

    // Increment flag for update.
    flag++;


}


//
// This would be used to write out the mixbuffer
//  during each game loop update.
// Updates sound buffer and audio device at runtime.
// It is called during Timer interrupt with SNDINTR.
// Mixing now done synchronous, and
//  only output be done asynchronous?
//
void
I_SubmitSound(void)
{
  // Write it to DSP device.
 // write(audio_fd, mixbuffer, SAMPLECOUNT*BUFMUL);

    currentAudioTime = SDL_GetTicks();
    if(currentAudioTime-oldAudioTime >= 30)
    {
    //    I_ClearSound();
   //     oldAudioTime = SDL_GetTicks();
    }
        SDL_PutAudioStreamData(mainAudio, mixbuffer, MIXBUFFERSIZE);


    if(SDL_GetAudioStreamQueued(mainAudio) < SAMPLECOUNT)
    {
//I_ClearSound();
     //
  //  SDL_PutAudioStreamData(mainAudio, mixbuffer, MIXBUFFERSIZE);

    }


}
void
I_ClearSound(void)
{

  SDL_ClearAudioStream(mainAudio);
}


void
I_UpdateSoundParams
( int	handle,
  int	vol,
  int	sep,
  int	pitch)
{
  // I fail too see that this is used.
  // Would be using the handle to identify
  //  on which channel the sound might be active,
  //  and resetting the channel parameters.

  // UNUSED.
  handle = vol = sep = pitch = 0;
}




void I_ShutdownSound(void)
{

    SDL_DestroyAudioStream(mainAudio);

  return;
}






void
I_InitSound()
{
SDL_Init(SDL_INIT_AUDIO);
  I_InitMusic();
  int i;
 SDL_AudioSpec input;

 input.format = SDL_AUDIO_S16;
 input.channels = 2;
 input.freq = SAMPLERATE;
    mainAudio = SDL_OpenAudioDeviceStream(SDL_AUDIO_DEVICE_DEFAULT_PLAYBACK,&input,NULL,NULL);
    if(mainAudio == NULL)
    {
        printf("\nAudio Device Stream not opened\n");
    }
SDL_ResumeAudioStreamDevice(mainAudio);

  // Initialize external data (all sounds) at start, keep static.
  fprintf( stderr, "I_InitSound: ");

  for (i=1 ; i<NUMSFX ; i++)
  {

    // Alias? Example is the chaingun sound linked to pistol.
    

        if (!S_sfx[i].link)
        {
          // Load data from WAD file.
          S_sfx[i].data = getsfx( S_sfx[i].name, &lengths[i] );
        }
        else
        {
          // Previously loaded already?
          S_sfx[i].data = S_sfx[i].link->data;
          lengths[i] = lengths[(S_sfx[i].link - S_sfx)/sizeof(sfxinfo_t)];
        }


  }

  fprintf( stderr, " pre-cached all sound data\n");

//  Now initialize mixbuffer with zero.
//  for ( i = 0; i< MIXBUFFERSIZE; i++ )
  //  mixbuffer[i] = 0;
  memset(mixbuffer, 0, MIXBUFFERSIZE);
  // Finished initialization.
  fprintf(stderr, "I_InitSound: sound module ready\n");


}




//
// MUSIC API.
//
void I_InitMusic(void)
{
  music_score_start = NULL;
  music_score_end = NULL;
  music_position = NULL;
  music_active = 0;
  music_paused = 0;
  MusicResetVoices();
}

void I_ShutdownMusic(void)
{
  music_active = 0;
  music_score_start = NULL;
  music_score_end = NULL;
}

void I_PlaySong(int handle, int looping)
{
  (void)handle;
  music_looping = looping;
  music_paused = 0;
  music_active = music_score_start != NULL;
  if (music_active) MusicRestart();
}

void I_PauseSong (int handle)
{
  (void)handle;
  music_paused = 1;
}

void I_ResumeSong (int handle)
{
  (void)handle;
  music_paused = 0;
}

void I_StopSong(int handle)
{
  (void)handle;
  music_active = 0;
  music_looping = 0;
  MusicResetVoices();
}

void I_UnRegisterSong(int handle)
{
  (void)handle;
  music_active = 0;
  music_score_start = NULL;
  music_score_end = NULL;
}

int I_RegisterSong(void* data)
{
  const unsigned char* bytes = (const unsigned char*)data;
  int score_length;
  int score_offset;
  music_active = 0;
  music_score_start = NULL;
  music_score_end = NULL;
  if (!bytes || memcmp(bytes, "MUS\x1a", 4) != 0)
  {
    fprintf(stderr, "I_RegisterSong: unsupported music format\n");
    return 0;
  }
  score_length = MusicRead16(bytes + 4);
  score_offset = MusicRead16(bytes + 6);
  if (score_length <= 0 || score_offset < 16)
    return 0;
  music_score_start = bytes + score_offset;
  music_score_end = music_score_start + score_length;
  MusicRestart();
  fprintf(stderr, "I_RegisterSong: MUS playback ready (%d-byte score)\n",
          score_length);
  return 1;
}

// Is the song playing?
int I_QrySongPlaying(int handle)
{
  (void)handle;
  return music_active;
}



//
// Experimental stuff.
// A Linux timer interrupt, for asynchronous
//  sound output.
// I ripped this out of the Timer class in
//  our Difference Engine, including a few
//  SUN remains...
//

    typedef     int             tSigSet;


// We might use SIGVTALRM and ITIMER_VIRTUAL, if the process
//  time independend timer happens to get lost due to heavy load.
// SIGALRM and ITIMER_REAL doesn't really work well.
// There are issues with profiling as well.


    /*
    static int   itimer = ITIMER_VIRTUAL;

static int sig = SIGALRM;
*/
// Interrupt handler.
void I_HandleSoundTimer( int ignore )
{
  // Debug.
  //fprintf( stderr, "%c", '+' ); fflush( stderr );

  // Feed sound device if necesary.
 
}

// Get the interrupt. Set duration in millisecs.
int I_SoundSetTimer( int duration_of_tick )
{
  // Needed for gametick clockwork.
/*
  struct itimerval    value;
  struct itimerval    ovalue;
  struct sigaction    act;
  struct sigaction    oact;

  int res;

  // This sets to SA_ONESHOT and SA_NOMASK, thus we can not use it.
  //     signal( _sig, handle_SIG_TICK );

  // Now we have to change this attribute for repeated calls.
  act.sa_handler = I_HandleSoundTimer;
#ifndef sun
  //ac	t.sa_mask = _sig;
#endif
  act.sa_flags = SA_RESTART;

  sigaction( sig, &act, &oact );

  value.it_interval.tv_sec    = 0;
  value.it_interval.tv_usec   = duration_of_tick;
  value.it_value.tv_sec       = 0;
  value.it_value.tv_usec      = duration_of_tick;

  // Error is -1.
  res = setitimer( itimer, &value, &ovalue );

  // Debug.
  if ( res == -1 )
    fprintf( stderr, "I_SoundSetTimer: interrupt n.a.\n");

  return res;
*/
return 0;
}


// Remove the interrupt. Set duration to zero.
void I_SoundDelTimer()
{

  // Debug.
  if ( I_SoundSetTimer( 0 ) == -1)
    fprintf( stderr, "I_SoundDelTimer: failed to remove interrupt. Doh!\n");

}
