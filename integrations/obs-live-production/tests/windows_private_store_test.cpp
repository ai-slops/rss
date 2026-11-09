#include "config.hpp"
#include "private_files_windows.hpp"
#include "recovery_store.hpp"

#include <QCoreApplication>
#include <QDir>
#include <QFile>
#include <QTemporaryDir>
#include <iostream>
#include <stdexcept>

using namespace live_production;

static void require(bool value, const char *message) {
  if (!value) throw std::runtime_error(message);
}

int main(int argc, char **argv) {
  QCoreApplication app(argc, argv);
  try {
    QTemporaryDir root;
    require(root.isValid(), "temporary directory");
    require(secure_windows_path(root.path().toStdString()), "secure directory");
    const auto bundle = root.filePath("publisher.json");
    QFile file(bundle);
    require(file.open(QIODevice::WriteOnly), "create bundle");
    file.write("{}"); file.close();
    std::string error;
    require(is_private_regular_file(bundle.toStdString(), error), "private inherited ACL");
    PSECURITY_DESCRIPTOR descriptor = nullptr;
    require(ConvertStringSecurityDescriptorToSecurityDescriptorW(L"D:P(A;;FA;;;WD)",
        SDDL_REVISION_1, &descriptor, nullptr), "Everyone descriptor");
    PACL dacl = nullptr; BOOL present = FALSE, defaulted = FALSE;
    GetSecurityDescriptorDacl(descriptor, &present, &dacl, &defaulted);
    auto wide = bundle.toStdWString();
    require(SetNamedSecurityInfoW(wide.data(), SE_FILE_OBJECT,
        DACL_SECURITY_INFORMATION | PROTECTED_DACL_SECURITY_INFORMATION,
        nullptr, nullptr, dacl, nullptr) == ERROR_SUCCESS, "broaden bundle ACL");
    LocalFree(descriptor);
    require(!is_private_regular_file(bundle.toStdString(), error), "reject Everyone access");
    require(secure_windows_path(bundle.toStdString()), "restore private bundle");
    RecoveryStore first, second;
    PersistedRecoveryState state{};
    const auto directory = root.filePath("recovery").toStdString();
    require(first.open(directory, bundle.toStdString(), "source-1", "controller-1", state, error), "first lock");
    require(!second.open(directory, bundle.toStdString(), "source-1", "controller-1", state, error), "reject duplicate lock");
    state.process_epoch = 7; state.poll_seq = 11; state.intent_epoch = 13;
    state.desired = LocalDesired::Running;
    state.last_command_id = "command-1"; state.last_command_result = ActionOutcome::Started;
    require(first.save(state, error), "persist state"); first.close();
    PersistedRecoveryState restored{};
    require(second.open(directory, bundle.toStdString(), "source-1", "controller-1", restored, error), "reopen state");
    require(restored.process_epoch == 7 && restored.poll_seq == 11 && restored.intent_epoch == 13 &&
        restored.desired == LocalDesired::Running && restored.last_command_id == "command-1" &&
        restored.last_command_result == ActionOutcome::Started, "restore exact state");
    second.close();
    require(first.open(directory, bundle.toStdString(), "source-1", "controller-2", restored, error), "new controller");
    require(restored.desired == LocalDesired::Stopped && restored.last_command_id.empty() &&
        !restored.last_command_result, "new controller cannot inherit running intent");
    std::cout << "Windows private ACL, exclusive recovery ownership and atomic state: PASS\n";
    return 0;
  } catch (const std::exception &error) { std::cerr << error.what() << '\n'; return 1; }
}
