//
// TanPropKeys.h - property keys for the TAN APO.
//
// Modeled on the WDK SwapAPO sample's CustomPropKeys.h (MIT license).
//
#pragma once

#include "propidl.h"

#ifdef DEFINE_PROPERTYKEY
#undef DEFINE_PROPERTYKEY
#endif

#ifdef INITGUID
#define DEFINE_PROPERTYKEY(name, l, w1, w2, b1, b2, b3, b4, b5, b6, b7, b8, pid) EXTERN_C const PROPERTYKEY name = { { l, w1, w2, { b1, b2,  b3,  b4,  b5,  b6,  b7,  b8 } }, pid }
#else
#define DEFINE_PROPERTYKEY(name, l, w1, w2, b1, b2, b3, b4, b5, b6, b7, b8, pid) EXTERN_C const PROPERTYKEY name
#endif // INITGUID

// PKEY_AudioEndpoint_Disable_SysFx, re-stated so the INITGUID TU emits the
// definition (mmdeviceapi.h is pulled in via ATL before INITGUID is set, so
// its own DEFINE_PROPERTYKEY only declares).
// {1DA5D803-D492-4EDD-8C23-E0C0FFEE7F0E},5
DEFINE_PROPERTYKEY(PKEY_AudioEndpoint_Disable_SysFx, 0x1da5d803, 0xd492, 0x4edd, 0x8c, 0x23, 0xe0, 0xc0, 0xff, 0xee, 0x7f, 0x0e, 5);

// PKEY_Endpoint_Enable_Tan_SFX: 0 disables TAN's leveling; absent or nonzero
// leaves it enabled (TAN defaults to on).
// {ADF13A0E-662A-4E2C-A574-D9BA77BA77C2},2
// vartype = VT_UI4
DEFINE_PROPERTYKEY(PKEY_Endpoint_Enable_Tan_SFX, 0xadf13a0e, 0x662a, 0x4e2c, 0xa5, 0x74, 0xd9, 0xba, 0x77, 0xba, 0x77, 0xc2, 2);
