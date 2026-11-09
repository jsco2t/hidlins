// Exercise the actual callback and dialogs through Flutter's public codec/messenger API.
#include <functional>
#include <iostream>
#include <stdexcept>
#include <string>

#include "../../app/linux/runner/my_application.cc"

struct TestMessenger {
  GObject parent_instance;
  FlBinaryMessengerMessageHandler handler;
  gpointer data;
  GDestroyNotify destroy;
  GBytes* response;
  unsigned replies;
};
struct TestMessengerClass { GObjectClass parent_class; };
static void test_messenger_interface(FlBinaryMessengerInterface* iface);
G_DEFINE_TYPE_WITH_CODE(TestMessenger, test_messenger, G_TYPE_OBJECT,
                       G_IMPLEMENT_INTERFACE(fl_binary_messenger_get_type(),
                                             test_messenger_interface))

static void set_handler(FlBinaryMessenger* raw, const gchar*,
                        FlBinaryMessengerMessageHandler handler, gpointer data,
                        GDestroyNotify destroy) {
  auto* self = reinterpret_cast<TestMessenger*>(raw);
  if (self->destroy != nullptr) self->destroy(self->data);
  self->handler = handler;
  self->data = data;
  self->destroy = destroy;
}

static gboolean send_response(FlBinaryMessenger* raw,
                               FlBinaryMessengerResponseHandle*, GBytes* bytes,
                               GError**) {
  auto* self = reinterpret_cast<TestMessenger*>(raw);
  self->replies++;
  g_clear_pointer(&self->response, g_bytes_unref);
  self->response = bytes == nullptr ? nullptr : g_bytes_ref(bytes);
  return TRUE;
}

static void test_messenger_interface(FlBinaryMessengerInterface* iface) {
  iface->set_message_handler_on_channel = set_handler;
  iface->send_response = send_response;
}
static void messenger_dispose(GObject* raw) {
  auto* self = reinterpret_cast<TestMessenger*>(raw);
  if (self->destroy != nullptr) self->destroy(self->data);
  self->handler = nullptr;
  self->data = nullptr;
  self->destroy = nullptr;
  g_clear_pointer(&self->response, g_bytes_unref);
  G_OBJECT_CLASS(test_messenger_parent_class)->dispose(raw);
}
static void test_messenger_class_init(TestMessengerClass* klass) {
  G_OBJECT_CLASS(klass)->dispose = messenger_dispose;
}
static void test_messenger_init(TestMessenger*) {}

struct TestResponseHandle { FlBinaryMessengerResponseHandle parent_instance; };
struct TestResponseHandleClass { FlBinaryMessengerResponseHandleClass parent_class; };
G_DEFINE_TYPE(TestResponseHandle, test_response_handle,
              fl_binary_messenger_response_handle_get_type())
static void test_response_handle_class_init(TestResponseHandleClass*) {}
static void test_response_handle_init(TestResponseHandle*) {}

static void require(bool value, const char* message) {
  if (!value) throw std::runtime_error(message);
}

class Channel {
 public:
  Channel() {
    messenger_ = reinterpret_cast<TestMessenger*>(g_object_new(test_messenger_get_type(), nullptr));
    app_ = my_application_new();
    codec_ = FL_METHOD_CODEC(fl_standard_method_codec_new());
    channel_ = fl_method_channel_new(FL_BINARY_MESSENGER(messenger_),
                                   "app.hidlins/attachment_export", codec_);
    fl_method_channel_set_method_call_handler(channel_, attachment_export_method_call_cb, app_, nullptr);
  }
  ~Channel() {
    // Break the real channel/messenger registration reference cycle explicitly.
    fl_binary_messenger_set_message_handler_on_channel(FL_BINARY_MESSENGER(messenger_),
        "app.hidlins/attachment_export", nullptr, nullptr, nullptr);
    g_object_unref(channel_);
    g_object_unref(codec_);
    g_object_unref(app_);
    g_object_unref(messenger_);
  }
  FlValue* invoke(const char* method, FlValue* args = nullptr) {
    g_clear_object(&response_);
    g_clear_pointer(&messenger_->response, g_bytes_unref);
    messenger_->replies = 0;
    g_autoptr(GError) error = nullptr;
    g_autoptr(GBytes) message = FL_METHOD_CODEC_GET_CLASS(codec_)->encode_method_call(codec_, method, args, &error);
    require(message != nullptr && error == nullptr, "Could not encode fixture method call");
    g_autoptr(FlBinaryMessengerResponseHandle) handle = FL_BINARY_MESSENGER_RESPONSE_HANDLE(
        g_object_new(test_response_handle_get_type(), nullptr));
    messenger_->handler(FL_BINARY_MESSENGER(messenger_), "app.hidlins/attachment_export",
                        message, handle, messenger_->data);
    require(messenger_->replies == 1, "Expected exactly one native response");
    if (messenger_->response == nullptr || g_bytes_get_size(messenger_->response) == 0) return nullptr;
    response_ = FL_METHOD_CODEC_GET_CLASS(codec_)->decode_response(codec_, messenger_->response, &error);
    require(response_ != nullptr && error == nullptr, "Malformed native method response");
    FlValue* result = fl_method_response_get_result(response_, &error);
    require(error == nullptr && result != nullptr && fl_value_get_type(result) == FL_VALUE_TYPE_MAP,
            "Expected a typed result envelope");
    return result;
  }
  void clear_response() { g_clear_object(&response_); }
 private:
  TestMessenger* messenger_;
  MyApplication* app_;
  FlMethodCodec* codec_;
  FlMethodChannel* channel_;
  FlMethodResponse* response_ = nullptr;
};

static void expect_envelope(FlValue* value, const char* status, const char* path = nullptr,
                             const char* code = nullptr) {
  require(value != nullptr, "Expected supported attachment operation; received NotImplemented");
  FlValue* found = fl_value_lookup_string(value, "status");
  require(found != nullptr && fl_value_get_type(found) == FL_VALUE_TYPE_STRING &&
          g_str_equal(fl_value_get_string(found), status), "Unexpected attachment result status");
  require(fl_value_get_length(value) == 1u + (path != nullptr) + (code != nullptr),
          "Unexpected envelope fields (only status, selected path or generic code permitted)");
  if (path != nullptr) {
    found = fl_value_lookup_string(value, "value");
    require(found != nullptr && fl_value_get_type(found) == FL_VALUE_TYPE_STRING &&
            g_str_equal(fl_value_get_string(found), path), "Selected path was not returned unchanged");
  }
  if (code != nullptr) {
    found = fl_value_lookup_string(value, "code");
    require(found != nullptr && fl_value_get_type(found) == FL_VALUE_TYPE_STRING &&
            g_str_equal(fl_value_get_string(found), code), "Unexpected generic failure code");
  }
}

struct DialogDriver {
  GtkFileChooserAction action;
  const char* path;
  gint response;
  bool seen = false;
  bool configured = false;
  std::string error;
  gint64 deadline = g_get_monotonic_time() + 5000000;
  guint timer;
  DialogDriver(GtkFileChooserAction expected, const char* selected, gint result)
      : action(expected), path(selected), response(result) {
    timer = g_timeout_add(20, tick, this);
  }
  ~DialogDriver() { g_source_remove(timer); }
  void verify() const {
    require(seen, "No actual GTK file chooser was opened");
    if (!error.empty()) throw std::runtime_error(error);
  }
  static gboolean tick(gpointer data) {
    auto* self = static_cast<DialogDriver*>(data);
    GList* windows = gtk_window_list_toplevels();
    for (GList* item = windows; item != nullptr; item = item->next) {
      if (!GTK_IS_FILE_CHOOSER_DIALOG(item->data)) continue;
      GtkDialog* dialog = GTK_DIALOG(item->data);
      GtkFileChooser* chooser = GTK_FILE_CHOOSER(item->data);
      self->seen = true;
      if (gtk_file_chooser_get_action(chooser) != self->action)
        self->error = "Wrong actual GTK chooser action";
      if (!gtk_file_chooser_get_local_only(chooser) || gtk_file_chooser_get_select_multiple(chooser))
        self->error = "Chooser must select one local filesystem path";
      if (self->action == GTK_FILE_CHOOSER_ACTION_OPEN && gtk_file_chooser_get_create_folders(chooser))
        self->error = "Source chooser must not create folders";
      if (self->action == GTK_FILE_CHOOSER_ACTION_SAVE && !gtk_file_chooser_get_do_overwrite_confirmation(chooser))
        self->error = "Existing SAVE overwrite confirmation was disabled";
      if (!self->configured && self->path != nullptr) {
        if (self->action == GTK_FILE_CHOOSER_ACTION_SAVE) {
          g_autofree gchar* directory = g_path_get_dirname(self->path);
          g_autofree gchar* name = g_path_get_basename(self->path);
          gtk_file_chooser_set_current_folder(chooser, directory);
          gtk_file_chooser_set_current_name(chooser, name);
        } else {
          gtk_file_chooser_set_filename(chooser, self->path);
        }
        self->configured = true;
      }
      g_autofree gchar* selected = gtk_file_chooser_get_filename(chooser);
      if (!self->error.empty() || g_get_monotonic_time() > self->deadline) {
        if (self->error.empty()) self->error = "GTK fixture did not become ready before deadline";
        gtk_dialog_response(dialog, GTK_RESPONSE_CANCEL);
      } else if (self->path == nullptr || g_strcmp0(selected, self->path) == 0) {
        gtk_dialog_response(dialog, self->response);
      }
      break;
    }
    g_list_free(windows);
    return G_SOURCE_CONTINUE;
  }
};

int main(int argc, char** argv) {
  if (argc != 3 || !gtk_init_check(nullptr, nullptr)) return 2;
  Channel channel;
  unsigned passed = 0, failed = 0;
  auto run = [&](const char* name, std::function<void()> body) {
    try { body(); ++passed; std::cout << "PASS " << name << std::endl; }
    catch (const std::exception& error) { ++failed; std::cerr << "FAIL " << name << ": " << error.what() << std::endl; }
    channel.clear_response();
  };
  run("source selected through actual GTK OPEN", [&] {
    DialogDriver driver(GTK_FILE_CHOOSER_ACTION_OPEN, argv[1], GTK_RESPONSE_ACCEPT);
    expect_envelope(channel.invoke("chooseSource"), "success", argv[1]);
    driver.verify();
  });
  run("source cancellation", [&] {
    DialogDriver driver(GTK_FILE_CHOOSER_ACTION_OPEN, nullptr, GTK_RESPONSE_CANCEL);
    expect_envelope(channel.invoke("chooseSource"), "canceled");
    driver.verify();
  });
  run("source window close", [&] {
    DialogDriver driver(GTK_FILE_CHOOSER_ACTION_OPEN, nullptr, GTK_RESPONSE_DELETE_EVENT);
    expect_envelope(channel.invoke("chooseSource"), "canceled");
    driver.verify();
  });
  run("source rejects arguments", [&] {
    g_autoptr(FlValue) args = fl_value_new_map();
    fl_value_set_string_take(args, "path", fl_value_new_string(argv[1]));
    expect_envelope(channel.invoke("chooseSource", args), "failure", nullptr, "invalid-source-arguments");
  });
  auto destination_args = []() {
    FlValue* args = fl_value_new_map();
    fl_value_set_string_take(args, "suggestedName", fl_value_new_string("attachment.bin"));
    return args;
  };
  run("existing SAVE selection", [&] {
    g_autoptr(FlValue) args = destination_args();
    DialogDriver driver(GTK_FILE_CHOOSER_ACTION_SAVE, argv[2], GTK_RESPONSE_ACCEPT);
    expect_envelope(channel.invoke("chooseDestination", args), "success", argv[2]);
    driver.verify();
  });
  run("existing SAVE cancellation", [&] {
    g_autoptr(FlValue) args = destination_args();
    DialogDriver driver(GTK_FILE_CHOOSER_ACTION_SAVE, nullptr, GTK_RESPONSE_CANCEL);
    expect_envelope(channel.invoke("chooseDestination", args), "canceled");
    driver.verify();
  });
  run("existing unsafe destination names", [&] {
    for (const char* name : {"", ".", "..", "../entry", "dir/entry", "dir\\entry", "line\nentry"}) {
      g_autoptr(FlValue) args = fl_value_new_map();
      fl_value_set_string_take(args, "suggestedName", fl_value_new_string(name));
      expect_envelope(channel.invoke("chooseDestination", args), "failure", nullptr, "invalid-suggested-name");
    }
    expect_envelope(channel.invoke("chooseDestination"), "failure", nullptr, "invalid-suggested-name");
  });
  run("unknown method stays unimplemented", [&] {
    require(channel.invoke("unknown") == nullptr, "Unexpected support for unknown method");
  });
  std::cout << passed << " passed; " << failed << " failed" << std::endl;
  return failed == 0 ? 0 : 1;
}
