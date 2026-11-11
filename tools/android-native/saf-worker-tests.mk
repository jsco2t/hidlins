.PHONY: test api29 api36 adb-ready
.NOTPARALLEL:

SAF_API29 := $(shell python3 tools/android-native/emulator_matrix.py select | awk '$$2 == 29 {print $$1 " " $$3}')
SAF_API36 := $(shell python3 tools/android-native/emulator_matrix.py select | awk '$$2 == 36 {print $$1 " " $$3}')
ifeq ($(strip $(SAF_API29) $(SAF_API36)),)
$(error The host must support the repository Android 29/36 emulator matrix)
endif

test: api29 api36

adb-ready:
	"$(or $(ANDROID_SDK_ROOT),$(ANDROID_HOME))/platform-tools/adb" -P 5039 start-server

# Explicit emulator serial and private ADB server exclude physical devices.
# The existing native runner owns boot, bounded instrumentation and cleanup.
api29: adb-ready
	$(MAKE) -f Makefile app-test-android-native HIDLINS_ANDROID_AVD=$(word 1,$(SAF_API29)) HIDLINS_ANDROID_EXPECTED_API=29 HIDLINS_ANDROID_EXPECTED_ABI=$(word 2,$(SAF_API29)) HIDLINS_ANDROID_EMULATOR_PORT=5584 HIDLINS_ANDROID_ADB_TIMEOUT_SECONDS=120 ANDROID_SERIAL=emulator-5584 ANDROID_ADB_SERVER_PORT=5039 ADB_SERVER_SOCKET=tcp:127.0.0.1:5039

api36: adb-ready
	$(MAKE) -f Makefile app-test-android-native HIDLINS_ANDROID_AVD=$(word 1,$(SAF_API36)) HIDLINS_ANDROID_EXPECTED_API=36 HIDLINS_ANDROID_EXPECTED_ABI=$(word 2,$(SAF_API36)) HIDLINS_ANDROID_EMULATOR_PORT=5584 HIDLINS_ANDROID_ADB_TIMEOUT_SECONDS=120 ANDROID_SERIAL=emulator-5584 ANDROID_ADB_SERVER_PORT=5039 ADB_SERVER_SOCKET=tcp:127.0.0.1:5039
