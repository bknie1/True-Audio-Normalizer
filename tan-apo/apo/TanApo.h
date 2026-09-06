//
// TanApo.h - declaration of the CTanApoSFX class.
//
// Derived from the Microsoft WDK SwapAPO sample (Windows-driver-samples,
// audio/sysvad/APO/SwapAPO, MIT license). The COM/APO plumbing follows the
// sample; the processing body is TAN's leveling engine via TanDsp.
//

#pragma once

#include <audioenginebaseapo.h>
#include <audioengineextensionapo.h>
#include <BaseAudioProcessingObject.h>
#include <TanApoInterface.h>
#include <TanApoDll.h>

#include <commonmacros.h>
#include <devicetopology.h>

#include <wil\com.h>

#include "TanDsp.h"

_Analysis_mode_(_Analysis_code_type_user_driver_)

#define PK_EQUAL(x, y)  ((x.fmtid == y.fmtid) && (x.pid == y.pid))
#define GUID_FORMAT_STRING "{%08x-%04x-%04x-%02x%02x-%02x%02x%02x%02x%02x%02x}"
#define GUID_FORMAT_ARGS(guidVal)  (guidVal).Data1, (guidVal).Data2, (guidVal).Data3, (guidVal).Data4[0], (guidVal).Data4[1], (guidVal).Data4[2], (guidVal).Data4[3], (guidVal).Data4[4], (guidVal).Data4[5], (guidVal).Data4[6], (guidVal).Data4[7]
#define NUM_OF_EFFECTS 1

//
// GUID identifying the TAN leveling effect exposed by this APO.
// {E8E830CD-6430-4E8D-AAD7-9438E8F77694}
DEFINE_GUID(TanEffectId, 0xe8e830cd, 0x6430, 0x4e8d, 0xaa, 0xd7, 0x94, 0x38, 0xe8, 0xf7, 0x76, 0x94);

// Activation context for the SFX property store.
// {F3B2E4B2-0E52-4469-AF15-3C1B3B33A77B}
DEFINE_GUID(TAN_APO_SFX_CONTEXT, 0xf3b2e4b2, 0x0e52, 0x4469, 0xaf, 0x15, 0x3c, 0x1b, 0x3b, 0x33, 0xa7, 0x7b);

LONG GetCurrentTanSetting(IPropertyStore* properties, PROPERTYKEY pkeyEnable, GUID processingMode);

#pragma AVRT_VTABLES_BEGIN
// TAN APO class - SFX
class CTanApoSFX :
    public CComObjectRootEx<CComMultiThreadModel>,
    public CComCoClass<CTanApoSFX, &CLSID_TanApoSFX>,
    public CBaseAudioProcessingObject,
    public IMMNotificationClient,
    public IAudioProcessingObjectNotifications,
    public IAudioSystemEffects3,
    public ITanApoSFX
{
public:
    // constructor
    CTanApoSFX()
    :   CBaseAudioProcessingObject(sm_RegProperties)
    ,   m_hEffectsChangedEvent(NULL)
    ,   m_AudioProcessingMode(AUDIO_SIGNALPROCESSINGMODE_DEFAULT)
    ,   m_fEnableTanSFX(FALSE)
    {
    }

    virtual ~CTanApoSFX();    // destructor

DECLARE_REGISTRY_RESOURCEID(IDR_TANAPOSFX)

BEGIN_COM_MAP(CTanApoSFX)
    COM_INTERFACE_ENTRY(ITanApoSFX)
    COM_INTERFACE_ENTRY(IAudioSystemEffects)
    COM_INTERFACE_ENTRY(IAudioSystemEffects2)
    COM_INTERFACE_ENTRY(IAudioSystemEffects3)
    COM_INTERFACE_ENTRY(IMMNotificationClient)
    COM_INTERFACE_ENTRY(IAudioProcessingObjectNotifications)
    COM_INTERFACE_ENTRY(IAudioProcessingObjectRT)
    COM_INTERFACE_ENTRY(IAudioProcessingObject)
    COM_INTERFACE_ENTRY(IAudioProcessingObjectConfiguration)
END_COM_MAP()

DECLARE_PROTECT_FINAL_CONSTRUCT()

public:
    STDMETHOD_(void, APOProcess)(UINT32 u32NumInputConnections,
        APO_CONNECTION_PROPERTY** ppInputConnections, UINT32 u32NumOutputConnections,
        APO_CONNECTION_PROPERTY** ppOutputConnections);

    STDMETHOD(GetLatency)(HNSTIME* pTime);

    STDMETHOD(LockForProcess)(UINT32 u32NumInputConnections,
        APO_CONNECTION_DESCRIPTOR** ppInputConnections,
        UINT32 u32NumOutputConnections, APO_CONNECTION_DESCRIPTOR** ppOutputConnections);

    STDMETHOD(UnlockForProcess)(void);

    STDMETHOD(Initialize)(UINT32 cbDataSize, BYTE* pbyData);

    // IAudioSystemEffects2
    STDMETHOD(GetEffectsList)(_Outptr_result_buffer_maybenull_(*pcEffects)  LPGUID *ppEffectsIds, _Out_ UINT *pcEffects, _In_ HANDLE Event);

    // IAudioSystemEffects3
    STDMETHOD(GetControllableSystemEffectsList)(_Outptr_result_buffer_maybenull_(*numEffects) AUDIO_SYSTEMEFFECT** effects, _Out_ UINT* numEffects, _In_opt_ HANDLE event);

    STDMETHOD(SetAudioSystemEffectState)(GUID effectId, AUDIO_SYSTEMEFFECT_STATE state);

    // IMMNotificationClient
    STDMETHODIMP OnDeviceStateChanged(LPCWSTR pwstrDeviceId, DWORD dwNewState)
    {
        UNREFERENCED_PARAMETER(pwstrDeviceId);
        UNREFERENCED_PARAMETER(dwNewState);
        return S_OK;
    }
    STDMETHODIMP OnDeviceAdded(LPCWSTR pwstrDeviceId)
    {
        UNREFERENCED_PARAMETER(pwstrDeviceId);
        return S_OK;
    }
    STDMETHODIMP OnDeviceRemoved(LPCWSTR pwstrDeviceId)
    {
        UNREFERENCED_PARAMETER(pwstrDeviceId);
        return S_OK;
    }
    STDMETHODIMP OnDefaultDeviceChanged(EDataFlow flow, ERole role, LPCWSTR pwstrDefaultDeviceId)
    {
        UNREFERENCED_PARAMETER(flow);
        UNREFERENCED_PARAMETER(role);
        UNREFERENCED_PARAMETER(pwstrDefaultDeviceId);
        return S_OK;
    }
    STDMETHODIMP OnPropertyValueChanged(LPCWSTR pwstrDeviceId, const PROPERTYKEY key);

    // IAudioProcessingObjectNotifications
    STDMETHODIMP GetApoNotificationRegistrationInfo(_Out_writes_(*count) APO_NOTIFICATION_DESCRIPTOR** apoNotifications, _Out_ DWORD* count);
    STDMETHODIMP_(void) HandleNotification(_In_ APO_NOTIFICATION* apoNotification);

public:
    LONG                                    m_fEnableTanSFX;
    GUID                                    m_AudioProcessingMode;
    wil::com_ptr_nothrow<IMMDevice>         m_deviceTopologyMMDevice;
    wil::com_ptr_nothrow<IMMDevice>         m_audioEndpoint;
    CComPtr<IPropertyStore>                 m_spAPOSystemEffectsProperties;
    CComPtr<IMMDeviceEnumerator>            m_spEnumerator;
    static const CRegAPOProperties<1>       sm_RegProperties;   // registration properties
    AUDIO_SYSTEMEFFECT                      m_effectInfos[NUM_OF_EFFECTS];

    CCriticalSection                        m_EffectsLock;
    HANDLE                                  m_hEffectsChangedEvent;

private:
    TanDsp m_tan;

    wil::com_ptr_nothrow<IPropertyStore> m_userStore;
    wil::com_ptr_nothrow<IAudioProcessingObjectLoggingService> m_apoLoggingService;
    BOOL m_bRegisteredEndpointNotificationCallback = FALSE;
};
#pragma AVRT_VTABLES_END

OBJECT_ENTRY_AUTO(__uuidof(TanApoSFX), CTanApoSFX)

//
// Convenience methods
//

void WriteSilence(
    _Out_writes_(u32FrameCount * u32SamplesPerFrame)
        FLOAT32 *pf32Frames,
    UINT32 u32FrameCount,
    UINT32 u32SamplesPerFrame );

void CopyFrames(
    _Out_writes_(u32FrameCount * u32SamplesPerFrame)
        FLOAT32 *pf32OutFrames,
    _In_reads_(u32FrameCount * u32SamplesPerFrame)
        const FLOAT32 *pf32InFrames,
    UINT32 u32FrameCount,
    UINT32 u32SamplesPerFrame );
