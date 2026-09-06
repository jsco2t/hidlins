#include "my_application.h"

#include <flutter_linux/flutter_linux.h>
#ifdef GDK_WINDOWING_X11
#include <gdk/gdkx.h>
#endif

#include "flutter/generated_plugin_registrant.h"

struct _MyApplication {
  GtkApplication parent_instance;
  char** dart_entrypoint_arguments;
  FlMethodChannel* attachment_export_channel;
};

G_DEFINE_TYPE(MyApplication, my_application, GTK_TYPE_APPLICATION)

static void set_hidlins_window_icon(GtkWindow* window) {
  g_autofree gchar* executable_path = g_file_read_link("/proc/self/exe", nullptr);
  if (executable_path == nullptr) {
    return;
  }
  g_autofree gchar* executable_dir = g_path_get_dirname(executable_path);
  g_autofree gchar* icon_path =
      g_build_filename(executable_dir, "data", "hidlins.png", nullptr);
  gtk_window_set_icon_from_file(window, icon_path, nullptr);
}

static FlMethodResponse* attachment_export_response(const gchar* status,
                                                    const gchar* value,
                                                    const gchar* code) {
  g_autoptr(FlValue) envelope = fl_value_new_map();
  fl_value_set_string_take(envelope, "status", fl_value_new_string(status));
  if (value != nullptr) {
    fl_value_set_string_take(envelope, "value", fl_value_new_string(value));
  }
  if (code != nullptr) {
    fl_value_set_string_take(envelope, "code", fl_value_new_string(code));
  }
  return FL_METHOD_RESPONSE(fl_method_success_response_new(envelope));
}

static gboolean is_safe_suggested_name(const gchar* value) {
  if (value == nullptr || value[0] == '\0' || g_str_equal(value, ".") ||
      g_str_equal(value, "..") || !g_utf8_validate(value, -1, nullptr)) {
    return FALSE;
  }
  for (const gchar* cursor = value; *cursor != '\0';
       cursor = g_utf8_next_char(cursor)) {
    const gunichar character = g_utf8_get_char(cursor);
    if (character == '/' || character == '\\' ||
        g_unichar_iscntrl(character)) {
      return FALSE;
    }
  }
  return TRUE;
}

static void attachment_export_method_call_cb(FlMethodChannel*,
                                             FlMethodCall* method_call,
                                             gpointer user_data) {
  g_autoptr(FlMethodResponse) response = nullptr;
  const gchar* method = fl_method_call_get_name(method_call);
  if (g_strcmp0(method, "chooseDestination") != 0) {
    response = FL_METHOD_RESPONSE(fl_method_not_implemented_response_new());
  } else {
    FlValue* arguments = fl_method_call_get_args(method_call);
    FlValue* suggested_name =
        arguments != nullptr && fl_value_get_type(arguments) == FL_VALUE_TYPE_MAP
            ? fl_value_lookup_string(arguments, "suggestedName")
            : nullptr;
    if (suggested_name == nullptr ||
        fl_value_get_type(suggested_name) != FL_VALUE_TYPE_STRING ||
        !is_safe_suggested_name(fl_value_get_string(suggested_name))) {
      response = attachment_export_response("failure", nullptr,
                                            "invalid-suggested-name");
    } else {
      MyApplication* self = MY_APPLICATION(user_data);
      GtkWindow* parent = gtk_application_get_active_window(
          GTK_APPLICATION(self));
      GtkWidget* dialog = gtk_file_chooser_dialog_new(
          "Save attachment", parent, GTK_FILE_CHOOSER_ACTION_SAVE, "_Cancel",
          GTK_RESPONSE_CANCEL, "_Save", GTK_RESPONSE_ACCEPT, nullptr);
      GtkFileChooser* chooser = GTK_FILE_CHOOSER(dialog);
      gtk_file_chooser_set_do_overwrite_confirmation(chooser, TRUE);
      gtk_file_chooser_set_current_name(chooser,
                                        fl_value_get_string(suggested_name));

      if (gtk_dialog_run(GTK_DIALOG(dialog)) == GTK_RESPONSE_ACCEPT) {
        g_autofree gchar* destination = gtk_file_chooser_get_filename(chooser);
        if (destination != nullptr && destination[0] != '\0') {
          response =
              attachment_export_response("success", destination, nullptr);
        } else {
          response = attachment_export_response("failure", nullptr,
                                                "save-dialog-failed");
        }
      } else {
        response = attachment_export_response("canceled", nullptr, nullptr);
      }
      gtk_widget_destroy(dialog);
    }
  }

  g_autoptr(GError) error = nullptr;
  if (!fl_method_call_respond(method_call, response, &error)) {
    g_warning("Failed to send attachment export response: %s",
              error->message);
  }
}

// Called when first Flutter frame received.
static void first_frame_cb(MyApplication* self, FlView* view) {
  gtk_widget_show(gtk_widget_get_toplevel(GTK_WIDGET(view)));
}

// Implements GApplication::activate.
static void my_application_activate(GApplication* application) {
  MyApplication* self = MY_APPLICATION(application);
  GtkWindow* window =
      GTK_WINDOW(gtk_application_window_new(GTK_APPLICATION(application)));
  set_hidlins_window_icon(window);

  // Use a header bar when running in GNOME as this is the common style used
  // by applications and is the setup most users will be using (e.g. Ubuntu
  // desktop).
  // If running on X and not using GNOME then just use a traditional title bar
  // in case the window manager does more exotic layout, e.g. tiling.
  // If running on Wayland assume the header bar will work (may need changing
  // if future cases occur).
  gboolean use_header_bar = TRUE;
#ifdef GDK_WINDOWING_X11
  GdkScreen* screen = gtk_window_get_screen(window);
  if (GDK_IS_X11_SCREEN(screen)) {
    const gchar* wm_name = gdk_x11_screen_get_window_manager_name(screen);
    if (g_strcmp0(wm_name, "GNOME Shell") != 0) {
      use_header_bar = FALSE;
    }
  }
#endif
  if (use_header_bar) {
    GtkHeaderBar* header_bar = GTK_HEADER_BAR(gtk_header_bar_new());
    gtk_widget_show(GTK_WIDGET(header_bar));
    gtk_header_bar_set_title(header_bar, "Hidlins");
    gtk_header_bar_set_show_close_button(header_bar, TRUE);
    gtk_window_set_titlebar(window, GTK_WIDGET(header_bar));
  } else {
    gtk_window_set_title(window, "Hidlins");
  }

  gtk_window_set_default_size(window, 1280, 720);

  g_autoptr(FlDartProject) project = fl_dart_project_new();
  fl_dart_project_set_dart_entrypoint_arguments(
      project, self->dart_entrypoint_arguments);

  FlView* view = fl_view_new(project);
  GdkRGBA background_color;
  // Background defaults to black, override it here if necessary, e.g. #00000000
  // for transparent.
  gdk_rgba_parse(&background_color, "#000000");
  fl_view_set_background_color(view, &background_color);
  gtk_widget_show(GTK_WIDGET(view));
  gtk_container_add(GTK_CONTAINER(window), GTK_WIDGET(view));

  // Show the window when Flutter renders.
  // Requires the view to be realized so we can start rendering.
  g_signal_connect_swapped(view, "first-frame", G_CALLBACK(first_frame_cb),
                           self);
  gtk_widget_realize(GTK_WIDGET(view));

  fl_register_plugins(FL_PLUGIN_REGISTRY(view));

  FlEngine* engine = fl_view_get_engine(view);
  FlBinaryMessenger* messenger = fl_engine_get_binary_messenger(engine);
  g_autoptr(FlStandardMethodCodec) codec = fl_standard_method_codec_new();
  self->attachment_export_channel = fl_method_channel_new(
      messenger, "app.hidlins/attachment_export", FL_METHOD_CODEC(codec));
  fl_method_channel_set_method_call_handler(
      self->attachment_export_channel, attachment_export_method_call_cb, self,
      nullptr);

  gtk_widget_grab_focus(GTK_WIDGET(view));
}

// Implements GApplication::local_command_line.
static gboolean my_application_local_command_line(GApplication* application,
                                                  gchar*** arguments,
                                                  int* exit_status) {
  MyApplication* self = MY_APPLICATION(application);
  // Strip out the first argument as it is the binary name.
  self->dart_entrypoint_arguments = g_strdupv(*arguments + 1);

  g_autoptr(GError) error = nullptr;
  if (!g_application_register(application, nullptr, &error)) {
    g_warning("Failed to register: %s", error->message);
    *exit_status = 1;
    return TRUE;
  }

  g_application_activate(application);
  *exit_status = 0;

  return TRUE;
}

// Implements GApplication::startup.
static void my_application_startup(GApplication* application) {
  // MyApplication* self = MY_APPLICATION(object);

  // Perform any actions required at application startup.

  G_APPLICATION_CLASS(my_application_parent_class)->startup(application);
}

// Implements GApplication::shutdown.
static void my_application_shutdown(GApplication* application) {
  // MyApplication* self = MY_APPLICATION(object);

  // Perform any actions required at application shutdown.

  G_APPLICATION_CLASS(my_application_parent_class)->shutdown(application);
}

// Implements GObject::dispose.
static void my_application_dispose(GObject* object) {
  MyApplication* self = MY_APPLICATION(object);
  g_clear_pointer(&self->dart_entrypoint_arguments, g_strfreev);
  g_clear_object(&self->attachment_export_channel);
  G_OBJECT_CLASS(my_application_parent_class)->dispose(object);
}

static void my_application_class_init(MyApplicationClass* klass) {
  G_APPLICATION_CLASS(klass)->activate = my_application_activate;
  G_APPLICATION_CLASS(klass)->local_command_line =
      my_application_local_command_line;
  G_APPLICATION_CLASS(klass)->startup = my_application_startup;
  G_APPLICATION_CLASS(klass)->shutdown = my_application_shutdown;
  G_OBJECT_CLASS(klass)->dispose = my_application_dispose;
}

static void my_application_init(MyApplication* self) {}

MyApplication* my_application_new() {
  // Set the program name to the application ID, which helps various systems
  // like GTK and desktop environments map this running application to its
  // corresponding .desktop file. This ensures better integration by allowing
  // the application to be recognized beyond its binary name.
  g_set_prgname(APPLICATION_ID);

  return MY_APPLICATION(g_object_new(my_application_get_type(),
                                     "application-id", APPLICATION_ID, "flags",
                                     G_APPLICATION_NON_UNIQUE, nullptr));
}
