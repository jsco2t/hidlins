.PHONY: test

STORAGE_JAVA := $(if $(JAVA_HOME),$(JAVA_HOME)/bin/)java
STORAGE_JAVAC := $(if $(JAVA_HOME),$(JAVA_HOME)/bin/)javac
STORAGE_ANDROID_JAR := $(or $(ANDROID_SDK_ROOT),$(ANDROID_HOME))/platforms/android-36/android.jar

test:
	@test -f "$(STORAGE_ANDROID_JAR)" || { echo "error: set ANDROID_SDK_ROOT to the provisioned SDK36" >&2; exit 1; }
	@set -eu; storage_classes="$$(mktemp -d)"; trap 'rm -rf "$$storage_classes"' EXIT; \
	"$(STORAGE_JAVAC)" --release 17 -Xlint:all -Werror -cp "$(STORAGE_ANDROID_JAR)" -d "$$storage_classes" app/android/app/src/main/java/app/hidlins/AppStorage.java tools/android-native/storage_directory/StorageDirectoryRace.java; \
	"$(STORAGE_JAVA)" -cp "$$storage_classes:$(STORAGE_ANDROID_JAR)" app.hidlins.StorageDirectoryRace
