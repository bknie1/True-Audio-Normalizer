// RAII wrapper around the tan-ffi streaming engine. This is the only
// TAN-specific state the APO holds: created in LockForProcess (where the audio
// format is known), used in APOProcess, destroyed in UnlockForProcess.
#pragma once
#include "../tan_ffi.h"

class TanDsp {
public:
    TanDsp() = default;

    // Movie profile = 0, Music = 1. The APO passes the endpoint's format.
    void Create(uint32_t sampleRate, uint32_t channels, uint32_t profileId) {
        Destroy();
        m_channels = channels;
        m_handle = tan_normalizer_new(sampleRate, channels, profileId);
    }

    // In-place level of one interleaved block. frameCount = samples per channel.
    void Process(float* interleaved, size_t frameCount) {
        if (m_handle && interleaved) {
            tan_normalizer_process(m_handle, interleaved, frameCount * m_channels);
        }
    }

    void Destroy() {
        if (m_handle) {
            tan_normalizer_free(m_handle);
            m_handle = nullptr;
        }
    }

    ~TanDsp() { Destroy(); }

    bool Ready() const { return m_handle != nullptr; }

private:
    TanNormalizer* m_handle = nullptr;
    uint32_t m_channels = 2;
};
