//
// TanApoDll.cpp - DLL exports for the TAN APO.
//
// Derived from the Microsoft WDK SwapAPO sample (MIT license).
//

#include <atlbase.h>
#include <atlcom.h>
#include <atlcoll.h>
#include <atlsync.h>
#include <mmreg.h>

#include "resource.h"
#include "TanApoDll.h"
#include <TanApo.h>

#include <TanApoDll_i.c>

//-------------------------------------------------------------------------
// Array of APO_REG_PROPERTIES structures implemented in this module.
//
APO_REG_PROPERTIES const *gCoreAPOs[] =
{
    &CTanApoSFX::sm_RegProperties.m_Properties
};

class CTanApoDllModule : public CAtlDllModuleT< CTanApoDllModule >
{
public :
    DECLARE_LIBID(LIBID_TanApoDlllib)
    DECLARE_REGISTRY_APPID_RESOURCEID(IDR_TANAPODLL, "{FAEC9F7D-4062-427A-AB30-CA5F322840A3}")
};

CTanApoDllModule _AtlModule;


extern "C" BOOL WINAPI DllMain(HINSTANCE /* hInstance */, DWORD dwReason, LPVOID lpReserved)
{
    if (DLL_PROCESS_ATTACH == dwReason)
    {
    }
    // do necessary cleanup only if the DLL is being unloaded dynamically
    else if ((DLL_PROCESS_DETACH == dwReason) && (NULL == lpReserved))
    {
    }

    return _AtlModule.DllMain(dwReason, lpReserved);
}


__control_entrypoint(DllExport)
STDAPI DllCanUnloadNow(void)
{
    return _AtlModule.DllCanUnloadNow();
}


_Check_return_
STDAPI DllGetClassObject(_In_ REFCLSID rclsid, _In_ REFIID riid, _Outptr_ LPVOID FAR* ppv)
{
    return _AtlModule.DllGetClassObject(rclsid, riid, ppv);
}
