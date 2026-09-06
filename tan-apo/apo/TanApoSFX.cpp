//
// TanApoSFX.cpp - implementation of CTanApoSFX.
//
// Derived from the Microsoft WDK SwapAPO sample (Windows-driver-samples,
// audio/sysvad/APO/SwapAPO, MIT license). The COM/APO plumbing follows the
// sample; the per-buffer channel swap is replaced with TAN's leveling engine.
//

#include <atlbase.h>
#include <atlcom.h>
#include <atlcoll.h>
#include <atlsync.h>
#include <mmreg.h>

#include <audioenginebaseapo.h>
#include <baseaudioprocessingobject.h>
#include <resource.h>

#include <float.h>

#include <initguid.h>
#include "TanApo.h"
#include "TanPropKeys.h"
#include <devicetopology.h>
#include <propvarutil.h>

// KSDATAFORMAT_SUBTYPE_IEEE_FLOAT, defined locally to avoid pulling ksmedia.h
// into this TU. Shared-mode APO buffers are 32-bit float; this is a guard.
static const GUID TAN_SUBTYPE_IEEE_FLOAT =
    { 0x00000003, 0x0000, 0x0010, { 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71 } };

// Static declaration of the APO_REG_PROPERTIES structure
// associated with this APO.
#pragma warning (disable : 4815)
const AVRT_DATA CRegAPOProperties<1> CTanApoSFX::sm_RegProperties(
    __uuidof(TanApoSFX),                            // clsid of this APO
    L"TanApoSFX",                                   // friendly name of this APO
    L"Copyright (c) Brandon Knieriem, MIT license", // copyright info
    1,                                              // major version #
    0,                                              // minor version #
    __uuidof(ITanApoSFX)                            // iid of primary interface
    );

#pragma AVRT_CODE_BEGIN
void WriteSilence(
    _Out_writes_(u32FrameCount * u32SamplesPerFrame)
        FLOAT32 *pf32Frames,
    UINT32 u32FrameCount,
    UINT32 u32SamplesPerFrame )
{
    ZeroMemory(pf32Frames, sizeof(FLOAT32) * u32FrameCount * u32SamplesPerFrame);
}
#pragma AVRT_CODE_END

#pragma AVRT_CODE_BEGIN
void CopyFrames(
    _Out_writes_(u32FrameCount * u32SamplesPerFrame)
        FLOAT32 *pf32OutFrames,
    _In_reads_(u32FrameCount * u32SamplesPerFrame)
        const FLOAT32 *pf32InFrames,
    UINT32 u32FrameCount,
    UINT32 u32SamplesPerFrame )
{
    CopyMemory(pf32OutFrames, pf32InFrames, sizeof(FLOAT32) * u32FrameCount * u32SamplesPerFrame);
}
#pragma AVRT_CODE_END

//-------------------------------------------------------------------------
// Effective on/off for the TAN effect. Differs from the SwapAPO sample in
// one deliberate way: TAN defaults to ON when its enable property has never
// been written. The Windows master disable (PKEY_AudioEndpoint_Disable_SysFx)
// and RAW mode still win.
LONG GetCurrentTanSetting(IPropertyStore* properties, PROPERTYKEY pkeyEnable, GUID processingMode)
{
    HRESULT hr;
    BOOL enabled;
    PROPVARIANT var;

    PropVariantInit(&var);

    // Check the master disable property defined by Windows.
    hr = properties->GetValue(PKEY_AudioEndpoint_Disable_SysFx, &var);
    enabled = (SUCCEEDED(hr)) && !((var.vt == VT_UI4) && (var.ulVal != 0));

    PropVariantClear(&var);

    // Check the APO's enable property; absent means enabled (default on).
    hr = properties->GetValue(pkeyEnable, &var);
    if (SUCCEEDED(hr) && (var.vt == VT_UI4))
    {
        enabled = enabled && (var.ulVal != 0);
    }

    PropVariantClear(&var);

    enabled = enabled && !IsEqualGUID(processingMode, AUDIO_SIGNALPROCESSINGMODE_RAW);

    return (LONG)enabled;
}

#pragma AVRT_CODE_BEGIN
//-------------------------------------------------------------------------
// Description:
//
//  Process one block. Runs on the audio engine's real-time thread: no
//  allocations, no locks, no blocking. TAN levels the buffer in place.
//
STDMETHODIMP_(void) CTanApoSFX::APOProcess(
    UINT32 u32NumInputConnections,
    APO_CONNECTION_PROPERTY** ppInputConnections,
    UINT32 u32NumOutputConnections,
    APO_CONNECTION_PROPERTY** ppOutputConnections)
{
    UNREFERENCED_PARAMETER(u32NumInputConnections);
    UNREFERENCED_PARAMETER(u32NumOutputConnections);

    FLOAT32 *pf32InputFrames, *pf32OutputFrames;

    ATLASSERT(m_bIsLocked);

    switch( ppInputConnections[0]->u32BufferFlags )
    {
        case BUFFER_INVALID:
        {
            ATLASSERT(false);  // invalid flag - should never occur.  don't do anything.
            break;
        }
        case BUFFER_VALID:
        case BUFFER_SILENT:
        {
            pf32InputFrames = reinterpret_cast<FLOAT32*>(ppInputConnections[0]->pBuffer);
            ATLASSERT( IS_VALID_TYPED_READ_POINTER(pf32InputFrames) );
            pf32OutputFrames = reinterpret_cast<FLOAT32*>(ppOutputConnections[0]->pBuffer);
            ATLASSERT( IS_VALID_TYPED_WRITE_POINTER(pf32OutputFrames) );

            if (BUFFER_SILENT == ppInputConnections[0]->u32BufferFlags)
            {
                WriteSilence( pf32InputFrames,
                              ppInputConnections[0]->u32ValidFrameCount,
                              GetSamplesPerFrame() );
            }
            else if (
                !IsEqualGUID(m_AudioProcessingMode, AUDIO_SIGNALPROCESSINGMODE_RAW) &&
                m_fEnableTanSFX &&
                m_tan.Ready()
            )
            {
                // Level the block in place (interleaved float32).
                m_tan.Process(pf32InputFrames, ppInputConnections[0]->u32ValidFrameCount);
            }

            // copy the memory only if there is an output connection, and input/output pointers are unequal
            if ( (0 != u32NumOutputConnections) &&
                  (ppOutputConnections[0]->pBuffer != ppInputConnections[0]->pBuffer) )
            {
                CopyFrames( pf32OutputFrames, pf32InputFrames,
                            ppInputConnections[0]->u32ValidFrameCount,
                            GetSamplesPerFrame() );
            }

            // pass along buffer flags and valid frame count
            ppOutputConnections[0]->u32BufferFlags = ppInputConnections[0]->u32BufferFlags;
            ppOutputConnections[0]->u32ValidFrameCount = ppInputConnections[0]->u32ValidFrameCount;

            break;
        }
        default:
        {
            ATLASSERT(false);  // invalid flag - should never occur
            break;
        }
    } // switch

} // APOProcess
#pragma AVRT_CODE_END

//-------------------------------------------------------------------------
STDMETHODIMP CTanApoSFX::GetLatency(HNSTIME* pTime)
{
    ASSERT_NONREALTIME();
    HRESULT hr = S_OK;

    IF_TRUE_ACTION_JUMP(NULL == pTime, hr = E_POINTER, Exit);

    // TAN's look-ahead limiter adds a small fixed delay; report zero for the
    // dev build (tracked in the README's RT caveat alongside the ring audit).
    *pTime = 0;

Exit:
    return hr;
}

//-------------------------------------------------------------------------
// Description:
//
//  Locks the APO for processing and creates the TAN engine now that the
//  negotiated format (sample rate, channel count) is known.
//
STDMETHODIMP CTanApoSFX::LockForProcess(UINT32 u32NumInputConnections,
    APO_CONNECTION_DESCRIPTOR** ppInputConnections,
    UINT32 u32NumOutputConnections, APO_CONNECTION_DESCRIPTOR** ppOutputConnections)
{
    ASSERT_NONREALTIME();
    HRESULT hr = S_OK;

    hr = CBaseAudioProcessingObject::LockForProcess(u32NumInputConnections,
        ppInputConnections, u32NumOutputConnections, ppOutputConnections);
    IF_FAILED_JUMP(hr, Exit);

    if (u32NumOutputConnections > 0 && ppOutputConnections[0]->pFormat != nullptr)
    {
        UNCOMPRESSEDAUDIOFORMAT format;
        if (SUCCEEDED(ppOutputConnections[0]->pFormat->GetUncompressedAudioFormat(&format)) &&
            IsEqualGUID(format.guidFormatType, TAN_SUBTYPE_IEEE_FLOAT))
        {
            const uint32_t profileId = 0; // 0 = movie (dialogue-forward). TODO:
                                          // surface as an FX property so the
                                          // tray/settings UI can switch it live.
            m_tan.Create((uint32_t)format.fFramesPerSecond,
                         format.dwSamplesPerFrame,
                         profileId);
        }
        // Non-float format: leave m_tan empty; APOProcess passes audio through.
    }

Exit:
    return hr;
}

//-------------------------------------------------------------------------
STDMETHODIMP CTanApoSFX::UnlockForProcess(void)
{
    ASSERT_NONREALTIME();
    m_tan.Destroy();
    return CBaseAudioProcessingObject::UnlockForProcess();
}

//-------------------------------------------------------------------------
// Description:
//
//  Generic initialization routine for APOs. See the SwapAPO sample for the
//  full commentary on APOInitSystemEffects3/2/1 handling.
//
HRESULT CTanApoSFX::Initialize(UINT32 cbDataSize, BYTE* pbyData)
{
    HRESULT                     hr = S_OK;
    CComPtr<IDeviceTopology>    spMyDeviceTopology;
    CComPtr<IConnector>         spMyConnector;
    GUID                        processingMode;

    IF_TRUE_ACTION_JUMP( ((NULL == pbyData) && (0 != cbDataSize)), hr = E_INVALIDARG, Exit);
    IF_TRUE_ACTION_JUMP( ((NULL != pbyData) && (0 == cbDataSize)), hr = E_INVALIDARG, Exit);

    if (cbDataSize == sizeof(APOInitSystemEffects3))
    {
        APOInitSystemEffects3* papoSysFxInit3 = (APOInitSystemEffects3*)pbyData;

        // Try to get the logging service; failure to log is not fatal.
        (void)papoSysFxInit3->pServiceProvider->QueryService(SID_AudioProcessingObjectLoggingService, IID_PPV_ARGS(&m_apoLoggingService));

        // Windows should pass a valid collection.
        ATLASSERT(papoSysFxInit3->pDeviceCollection != nullptr);
        IF_TRUE_ACTION_JUMP(papoSysFxInit3->pDeviceCollection == nullptr, hr = E_INVALIDARG, Exit);

        IMMDeviceCollection* deviceCollection = reinterpret_cast<APOInitSystemEffects3*>(pbyData)->pDeviceCollection;
        UINT32 numDevices;

        // Get the endpoint on which this APO has been created
        // (it is the last device in the device collection).
        hr = deviceCollection->GetCount(&numDevices);
        IF_FAILED_JUMP(hr, Exit);

        hr = numDevices > 0 ? S_OK : E_UNEXPECTED;
        IF_FAILED_JUMP(hr, Exit);

        hr = deviceCollection->Item(numDevices - 1, &m_audioEndpoint);
        IF_FAILED_JUMP(hr, Exit);

        wil::unique_prop_variant activationParam;
        hr = InitPropVariantFromCLSID(TAN_APO_SFX_CONTEXT, &activationParam);
        IF_FAILED_JUMP(hr, Exit);

        wil::com_ptr_nothrow<IAudioSystemEffectsPropertyStore> effectsPropertyStore;
        hr = m_audioEndpoint->Activate(__uuidof(effectsPropertyStore), CLSCTX_ALL, &activationParam, effectsPropertyStore.put_void());
        IF_FAILED_JUMP(hr, Exit);

        hr = effectsPropertyStore->OpenUserPropertyStore(STGM_READ, m_userStore.put());
        IF_FAILED_JUMP(hr, Exit);

        hr = papoSysFxInit3->pDeviceCollection->Item(papoSysFxInit3->nSoftwareIoDeviceInCollection, &m_deviceTopologyMMDevice);
        IF_FAILED_JUMP(hr, Exit);

        hr = m_deviceTopologyMMDevice->Activate(__uuidof(IDeviceTopology), CLSCTX_ALL, NULL, (void**)&spMyDeviceTopology);
        IF_FAILED_JUMP(hr, Exit);

        hr = spMyDeviceTopology->GetConnector(papoSysFxInit3->nSoftwareIoConnectorIndex, &spMyConnector);
        IF_FAILED_JUMP(hr, Exit);

        processingMode = papoSysFxInit3->AudioProcessingMode;
    }
    else if (cbDataSize == sizeof(APOInitSystemEffects2))
    {
        APOInitSystemEffects2* papoSysFxInit2 = (APOInitSystemEffects2*)pbyData;

        m_spAPOSystemEffectsProperties = papoSysFxInit2->pAPOSystemEffectsProperties;

        ATLASSERT(papoSysFxInit2->pDeviceCollection != nullptr);
        IF_TRUE_ACTION_JUMP(papoSysFxInit2->pDeviceCollection == nullptr, hr = E_INVALIDARG, Exit);

        hr = papoSysFxInit2->pDeviceCollection->Item(papoSysFxInit2->nSoftwareIoDeviceInCollection, &m_deviceTopologyMMDevice);
        IF_FAILED_JUMP(hr, Exit);

        hr = m_deviceTopologyMMDevice->Activate(__uuidof(IDeviceTopology), CLSCTX_ALL, NULL, (void**)&spMyDeviceTopology);
        IF_FAILED_JUMP(hr, Exit);

        hr = spMyDeviceTopology->GetConnector(papoSysFxInit2->nSoftwareIoConnectorIndex, &spMyConnector);
        IF_FAILED_JUMP(hr, Exit);

        processingMode = papoSysFxInit2->AudioProcessingMode;
    }
    else if (cbDataSize == sizeof(APOInitSystemEffects))
    {
        APOInitSystemEffects* papoSysFxInit = (APOInitSystemEffects*)pbyData;

        m_spAPOSystemEffectsProperties = papoSysFxInit->pAPOSystemEffectsProperties;

        processingMode = AUDIO_SIGNALPROCESSINGMODE_DEFAULT;
    }
    else
    {
        // Invalid initialization size
        hr = E_INVALIDARG;
        goto Exit;
    }

    // Validate then save the processing mode.
    IF_TRUE_ACTION_JUMP((processingMode != AUDIO_SIGNALPROCESSINGMODE_DEFAULT        &&
                         processingMode != AUDIO_SIGNALPROCESSINGMODE_RAW            &&
                         processingMode != AUDIO_SIGNALPROCESSINGMODE_COMMUNICATIONS &&
                         processingMode != AUDIO_SIGNALPROCESSINGMODE_SPEECH         &&
                         processingMode != AUDIO_SIGNALPROCESSINGMODE_MEDIA          &&
                         processingMode != AUDIO_SIGNALPROCESSINGMODE_MOVIE          &&
                         processingMode != AUDIO_SIGNALPROCESSINGMODE_NOTIFICATION), hr = E_INVALIDARG, Exit);
    m_AudioProcessingMode = processingMode;

    //
    // Get the current enable state.
    //
    if (m_userStore != nullptr)
    {
        m_fEnableTanSFX = GetCurrentTanSetting(m_userStore.get(), PKEY_Endpoint_Enable_Tan_SFX, m_AudioProcessingMode);
    }

    if (m_spAPOSystemEffectsProperties != NULL)
    {
        m_fEnableTanSFX = GetCurrentTanSetting(m_spAPOSystemEffectsProperties, PKEY_Endpoint_Enable_Tan_SFX, m_AudioProcessingMode);
    }

    RtlZeroMemory(m_effectInfos, sizeof(m_effectInfos));
    m_effectInfos[0] = { TanEffectId, TRUE, m_fEnableTanSFX ? AUDIO_SYSTEMEFFECT_STATE_ON : AUDIO_SYSTEMEFFECT_STATE_OFF };

    if (cbDataSize != sizeof(APOInitSystemEffects3))
    {
        //
        // Register for notification of registry updates
        //
        hr = m_spEnumerator.CoCreateInstance(__uuidof(MMDeviceEnumerator));
        IF_FAILED_JUMP(hr, Exit);

        hr = m_spEnumerator->RegisterEndpointNotificationCallback(this);
        IF_FAILED_JUMP(hr, Exit);

        m_bRegisteredEndpointNotificationCallback = TRUE;
    }

    m_bIsInitialized = true;

Exit:
    return hr;
}

//-------------------------------------------------------------------------
STDMETHODIMP CTanApoSFX::GetEffectsList(_Outptr_result_buffer_maybenull_(*pcEffects) LPGUID *ppEffectsIds, _Out_ UINT *pcEffects, _In_ HANDLE Event)
{
    HRESULT hr;
    BOOL effectsLocked = FALSE;
    UINT cEffects = 0;

    IF_TRUE_ACTION_JUMP(ppEffectsIds == NULL, hr = E_POINTER, Exit);
    IF_TRUE_ACTION_JUMP(pcEffects == NULL, hr = E_POINTER, Exit);

    // Synchronize access to the effects list and effects changed event
    m_EffectsLock.Enter();
    effectsLocked = TRUE;

    // Always close existing effects change event handle
    if (m_hEffectsChangedEvent != NULL)
    {
        CloseHandle(m_hEffectsChangedEvent);
        m_hEffectsChangedEvent = NULL;
    }

    // If an event handle was specified, save it here (duplicated to control lifetime)
    if (Event != NULL)
    {
        if (!DuplicateHandle(GetCurrentProcess(), Event, GetCurrentProcess(), &m_hEffectsChangedEvent, EVENT_MODIFY_STATE, FALSE, 0))
        {
            hr = HRESULT_FROM_WIN32(GetLastError());
            goto Exit;
        }
    }

    {
        struct EffectControl
        {
            GUID effect;
            BOOL control;
        };

        EffectControl list[] =
        {
            { TanEffectId,  m_fEnableTanSFX  },
        };

        if (!IsEqualGUID(m_AudioProcessingMode, AUDIO_SIGNALPROCESSINGMODE_RAW))
        {
            for (UINT i = 0; i < ARRAYSIZE(list); i++)
            {
                if (list[i].control)
                {
                    cEffects++;
                }
            }
        }

        if (0 == cEffects)
        {
            *ppEffectsIds = NULL;
            *pcEffects = 0;
        }
        else
        {
            GUID *pEffectsIds = (LPGUID)CoTaskMemAlloc(sizeof(GUID) * cEffects);
            if (pEffectsIds == nullptr)
            {
                hr = E_OUTOFMEMORY;
                goto Exit;
            }

            UINT j = 0;
            for (UINT i = 0; i < ARRAYSIZE(list); i++)
            {
                if (list[i].control)
                {
                    pEffectsIds[j++] = list[i].effect;
                }
            }

            *ppEffectsIds = pEffectsIds;
            *pcEffects = cEffects;
        }

        hr = S_OK;
    }

Exit:
    if (effectsLocked)
    {
        m_EffectsLock.Leave();
    }
    return hr;
}

HRESULT CTanApoSFX::GetControllableSystemEffectsList(_Outptr_result_buffer_maybenull_(*numEffects) AUDIO_SYSTEMEFFECT** effects, _Out_ UINT* numEffects, _In_opt_ HANDLE event)
{
    RETURN_HR_IF_NULL(E_POINTER, effects);
    RETURN_HR_IF_NULL(E_POINTER, numEffects);

    *effects = nullptr;
    *numEffects = 0;

    // Always close existing effects change event handle
    if (m_hEffectsChangedEvent != NULL)
    {
        CloseHandle(m_hEffectsChangedEvent);
        m_hEffectsChangedEvent = NULL;
    }

    // If an event handle was specified, save it here (duplicated to control lifetime)
    if (event != NULL)
    {
        if (!DuplicateHandle(GetCurrentProcess(), event, GetCurrentProcess(), &m_hEffectsChangedEvent, EVENT_MODIFY_STATE, FALSE, 0))
        {
            RETURN_IF_FAILED(HRESULT_FROM_WIN32(GetLastError()));
        }
    }

    if (!IsEqualGUID(m_AudioProcessingMode, AUDIO_SIGNALPROCESSINGMODE_RAW))
    {
        wil::unique_cotaskmem_array_ptr<AUDIO_SYSTEMEFFECT> audioEffects(
            static_cast<AUDIO_SYSTEMEFFECT*>(CoTaskMemAlloc(NUM_OF_EFFECTS * sizeof(AUDIO_SYSTEMEFFECT))), NUM_OF_EFFECTS);
        RETURN_IF_NULL_ALLOC(audioEffects.get());

        for (UINT i = 0; i < NUM_OF_EFFECTS; i++)
        {
            audioEffects[i].id = m_effectInfos[i].id;
            audioEffects[i].state = m_effectInfos[i].state;
            audioEffects[i].canSetState = m_effectInfos[i].canSetState;
        }

        *numEffects = (UINT)audioEffects.size();
        *effects = audioEffects.release();
    }

    return S_OK;
}

HRESULT CTanApoSFX::SetAudioSystemEffectState(GUID effectId, AUDIO_SYSTEMEFFECT_STATE state)
{
    for (auto effectInfo : m_effectInfos)
    {
        if (effectId == effectInfo.id)
        {
            AUDIO_SYSTEMEFFECT_STATE oldState = effectInfo.state;
            effectInfo.state = state;

            // Synchronize access to the effects list and effects changed event
            m_EffectsLock.Enter();

            // If anything changed and a change event handle exists
            if (oldState != effectInfo.state)
            {
                SetEvent(m_hEffectsChangedEvent);
                if (m_apoLoggingService != nullptr)
                {
                    m_apoLoggingService->ApoLog(APO_LOG_LEVEL_INFO, L"CTanApoSFX::SetAudioSystemEffectState - effect: " GUID_FORMAT_STRING L", state: %i", GUID_FORMAT_ARGS(effectInfo.id), effectInfo.state);
                }
            }

            m_EffectsLock.Leave();

            return S_OK;
        }
    }

    return E_NOTFOUND;
}

//-------------------------------------------------------------------------
HRESULT CTanApoSFX::OnPropertyValueChanged(LPCWSTR pwstrDeviceId, const PROPERTYKEY key)
{
    HRESULT     hr = S_OK;

    UNREFERENCED_PARAMETER(pwstrDeviceId);

    if (!m_spAPOSystemEffectsProperties)
    {
        return hr;
    }

    // If either the master disable or our APO's enable properties changed...
    if (PK_EQUAL(key, PKEY_Endpoint_Enable_Tan_SFX) ||
        PK_EQUAL(key, PKEY_AudioEndpoint_Disable_SysFx))
    {
        LONG nChanges = 0;

        // Synchronize access to the effects list and effects changed event
        m_EffectsLock.Enter();

        struct KeyControl
        {
            PROPERTYKEY key;
            LONG *value;
        };

        KeyControl controls[] =
        {
            { PKEY_Endpoint_Enable_Tan_SFX, &m_fEnableTanSFX  },
        };

        for (int i = 0; i < ARRAYSIZE(controls); i++)
        {
            LONG fOldValue;
            LONG fNewValue = true;

            fNewValue = GetCurrentTanSetting(m_spAPOSystemEffectsProperties, controls[i].key, m_AudioProcessingMode);

            // Swap in the new setting
            fOldValue = InterlockedExchange(controls[i].value, fNewValue);

            if (fNewValue != fOldValue)
            {
                nChanges++;
            }
        }

        // If anything changed and a change event handle exists
        if ((nChanges > 0) && (m_hEffectsChangedEvent != NULL))
        {
            SetEvent(m_hEffectsChangedEvent);
        }

        m_EffectsLock.Leave();
    }

    return hr;
}

HRESULT CTanApoSFX::GetApoNotificationRegistrationInfo(_Out_writes_(*count) APO_NOTIFICATION_DESCRIPTOR **apoNotifications, _Out_ DWORD *count)
{
    *apoNotifications = nullptr;
    *count = 0;

    constexpr DWORD numDescriptors = 1;
    wil::unique_cotaskmem_ptr<APO_NOTIFICATION_DESCRIPTOR[]> apoNotificationDescriptors;

    apoNotificationDescriptors.reset(static_cast<APO_NOTIFICATION_DESCRIPTOR*>(
        CoTaskMemAlloc(sizeof(APO_NOTIFICATION_DESCRIPTOR) * numDescriptors)));
    RETURN_IF_NULL_ALLOC(apoNotificationDescriptors);

    // Get notified when an endpoint property changes on the audio endpoint.
    apoNotificationDescriptors[0].type = APO_NOTIFICATION_TYPE_ENDPOINT_PROPERTY_CHANGE;
    (void)m_audioEndpoint.query_to(&apoNotificationDescriptors[0].audioEndpointPropertyChange.device);

    *apoNotifications = apoNotificationDescriptors.release();
    *count = numDescriptors;

    return S_OK;
}

void CTanApoSFX::HandleNotification(APO_NOTIFICATION *apoNotification)
{
    if (apoNotification->type == APO_NOTIFICATION_TYPE_ENDPOINT_PROPERTY_CHANGE)
    {
        // If either the master disable or our APO's enable properties changed...
        if (PK_EQUAL(apoNotification->audioEndpointPropertyChange.propertyKey, PKEY_Endpoint_Enable_Tan_SFX) ||
            PK_EQUAL(apoNotification->audioEndpointPropertyChange.propertyKey, PKEY_AudioEndpoint_Disable_SysFx))
        {
            struct KeyControl
            {
                PROPERTYKEY key;
                LONG* value;
            };

            KeyControl controls[] = {
                {PKEY_Endpoint_Enable_Tan_SFX, &m_fEnableTanSFX},
            };

            if (m_apoLoggingService != nullptr)
            {
                m_apoLoggingService->ApoLog(APO_LOG_LEVEL_INFO, L"CTanApoSFX::HandleNotification - pkey: " GUID_FORMAT_STRING L" %d", GUID_FORMAT_ARGS(apoNotification->audioEndpointPropertyChange.propertyKey.fmtid), apoNotification->audioEndpointPropertyChange.propertyKey.pid);
            }

            for (int i = 0; i < ARRAYSIZE(controls); i++)
            {
                LONG fNewValue = true;

                fNewValue = GetCurrentTanSetting(m_userStore.get(), controls[i].key, m_AudioProcessingMode);

                SetAudioSystemEffectState(m_effectInfos[i].id, fNewValue ? AUDIO_SYSTEMEFFECT_STATE_ON : AUDIO_SYSTEMEFFECT_STATE_OFF);
            }
        }
    }
}

//-------------------------------------------------------------------------
CTanApoSFX::~CTanApoSFX(void)
{
    //
    // unregister for callbacks
    //
    if (m_bRegisteredEndpointNotificationCallback)
    {
        m_spEnumerator->UnregisterEndpointNotificationCallback(this);
    }

    if (m_hEffectsChangedEvent != NULL)
    {
        CloseHandle(m_hEffectsChangedEvent);
    }
} // ~CTanApoSFX
