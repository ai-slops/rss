#pragma once

#ifdef _WIN32
#include <windows.h>
#include <aclapi.h>
#include <sddl.h>
#include <QString>
#include <string>
#include <vector>

namespace live_production {

inline std::vector<unsigned char> windows_user() {
  HANDLE token = nullptr;
  if (!OpenProcessToken(GetCurrentProcess(), TOKEN_QUERY, &token)) return {};
  DWORD size = 0;
  GetTokenInformation(token, TokenUser, nullptr, 0, &size);
  std::vector<unsigned char> bytes(size);
  if (!size || !GetTokenInformation(token, TokenUser, bytes.data(), size, &size)) bytes.clear();
  CloseHandle(token);
  return bytes;
}

inline bool private_windows_path(const std::string &path, bool directory, std::string &error) {
  const auto wide = QString::fromStdString(path).toStdWString();
  const DWORD attributes = GetFileAttributesW(wide.c_str());
  if (attributes == INVALID_FILE_ATTRIBUTES || (attributes & FILE_ATTRIBUTE_REPARSE_POINT) ||
      bool(attributes & FILE_ATTRIBUTE_DIRECTORY) != directory) {
    error = "private path must be a regular file or directory without reparse points";
    return false;
  }
  auto user = windows_user();
  if (user.empty()) { error = "cannot identify Windows user"; return false; }
  PSID owner = nullptr;
  PACL dacl = nullptr;
  PSECURITY_DESCRIPTOR descriptor = nullptr;
  if (GetNamedSecurityInfoW(wide.c_str(), SE_FILE_OBJECT,
      OWNER_SECURITY_INFORMATION | DACL_SECURITY_INFORMATION, &owner, nullptr, &dacl, nullptr,
      &descriptor) != ERROR_SUCCESS) {
    error = "cannot inspect private Windows file permissions";
    return false;
  }
  bool safe = owner && dacl &&
      (EqualSid(owner, reinterpret_cast<TOKEN_USER *>(user.data())->User.Sid) ||
       IsWellKnownSid(owner, WinBuiltinAdministratorsSid));
  for (DWORD index = 0; safe && index < dacl->AceCount; ++index) {
    void *entry = nullptr;
    if (!GetAce(dacl, index, &entry)) { safe = false; break; }
    const auto *header = static_cast<ACE_HEADER *>(entry);
    if (header->AceType == ACCESS_DENIED_ACE_TYPE) continue;
    if (header->AceType != ACCESS_ALLOWED_ACE_TYPE) { safe = false; break; }
    auto *ace = static_cast<ACCESS_ALLOWED_ACE *>(entry);
    PSID sid = reinterpret_cast<PSID>(&ace->SidStart);
    safe = EqualSid(sid, reinterpret_cast<TOKEN_USER *>(user.data())->User.Sid) ||
           IsWellKnownSid(sid, WinLocalSystemSid) || IsWellKnownSid(sid, WinBuiltinAdministratorsSid);
  }
  LocalFree(descriptor);
  if (!safe) error = "private Windows path must be owned by this user or Administrators and accessible only to this user, SYSTEM and Administrators";
  return safe;
}

inline bool secure_windows_path(const std::string &path) {
  auto user = windows_user();
  if (user.empty()) return false;
  LPWSTR sid = nullptr;
  if (!ConvertSidToStringSidW(reinterpret_cast<TOKEN_USER *>(user.data())->User.Sid, &sid)) return false;
  const std::wstring sddl = std::wstring(L"D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FA;;;") + sid + L")";
  LocalFree(sid);
  PSECURITY_DESCRIPTOR descriptor = nullptr;
  if (!ConvertStringSecurityDescriptorToSecurityDescriptorW(sddl.c_str(), SDDL_REVISION_1,
                                                            &descriptor, nullptr)) return false;
  BOOL present = FALSE, defaulted = FALSE;
  PACL dacl = nullptr;
  GetSecurityDescriptorDacl(descriptor, &present, &dacl, &defaulted);
  auto wide = QString::fromStdString(path).toStdWString();
  const bool ok = present && SetNamedSecurityInfoW(wide.data(), SE_FILE_OBJECT,
      DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
      nullptr, nullptr, dacl, nullptr) == ERROR_SUCCESS;
  LocalFree(descriptor);
  return ok;
}

}
#endif
