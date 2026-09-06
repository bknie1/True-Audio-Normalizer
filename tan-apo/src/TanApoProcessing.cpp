// The TAN-specific bodies to graft into a SwapAPO-derived System-Effect APO
// (from the WDK Windows-driver-samples SwapAPO / sysvad sample). These are NOT
// standalone - they show exactly what to change in the sample's SFX APO class.
// Keep all the sample's COM registration, class factory, and property-store
// plumbing; only these three methods carry TAN's behavior.
//
// Assumes the APO negotiated 32-bit float (KSDATAFORMAT_SUBTYPE_IEEE_FLOAT),
// which is the default for shared-mode SFX/MFX APOs. TanDsp is a member of the
// APO class (add `#include "TanDsp.h"` and `TanDsp m_tan;` to the header).

#include "TanDsp.h"
#include <audioenginebaseapo.h>

// --- In LockForProcess: build the engine now that the format is known. ---
// Inside CSwapAPOSFX::LockForProcess(...), after the base call succeeds and
// after you've captured the format (m_pafx*/m_uChannels etc. in the sample):
//
//   const WAVEFORMATEX* wfx = /* the negotiated output format */;
//   const uint32_t profileId = 0; // 0 = movie (dialogue-forward). TODO: make
//                                 // this a per-endpoint FX property so the tray
//                                 // / settings UI can switch it live.
//   m_tan.Create(wfx->nSamplesPerSec, wfx->nChannels, profileId);
//   return S_OK;

// --- The real-time processing method. Replace the sample's swap loop. ---
// CSwapAPOSFX::APOProcess signature matches the sample; this is the body.
//
// STDMETHODIMP_(void) CSwapAPOSFX::APOProcess(
//     UINT32 u32NumInputConnections,
//     APO_CONNECTION_PROPERTY** ppInputConnections,
//     UINT32 u32NumOutputConnections,
//     APO_CONNECTION_PROPERTY** ppOutputConnections)
// {
//     if (u32NumInputConnections == 0 || u32NumOutputConnections == 0) return;
//     APO_CONNECTION_PROPERTY* in  = ppInputConnections[0];
//     APO_CONNECTION_PROPERTY* out = ppOutputConnections[0];
//
//     switch (in->u32BufferFlags) {
//     case BUFFER_INVALID:
//         out->u32ValidFrameCount = 0;
//         break;
//     case BUFFER_SILENT:
//         // Nothing to level; forward silence and don't disturb the engine.
//         out->u32ValidFrameCount = in->u32ValidFrameCount;
//         out->u32BufferFlags = in->u32BufferFlags;
//         // (SFX APOs process in place: in and out share the buffer.)
//         break;
//     case BUFFER_VALID: {
//         float* pf32 = reinterpret_cast<float*>(in->pBuffer);
//         // TAN levels the block in place (interleaved float, per-channel
//         // frames = u32ValidFrameCount). TanDsp multiplies by channel count.
//         m_tan.Process(pf32, in->u32ValidFrameCount);
//         out->u32ValidFrameCount = in->u32ValidFrameCount;
//         out->u32BufferFlags = in->u32BufferFlags;
//         break;
//     }
//     }
// }

// --- In UnlockForProcess: tear the engine down. ---
// Inside CSwapAPOSFX::UnlockForProcess(), before the base call:
//   m_tan.Destroy();

// NOTE (real-time safety): m_tan.Process calls into Rust; Normalizer::process
// is in-place and allocation-free in steady state, but audit before shipping -
// the APOProcess thread must never allocate, lock, or block. See README.
