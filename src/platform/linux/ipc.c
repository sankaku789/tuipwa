/* GTK action transport only. Manifest / LaunchRequest have no GLib dependency. */
#include <gio/gio.h>
#include <stdio.h>
#include <unistd.h>
#define HOST "org.tuipwa.NativeHost"
#define PATH "/org/tuipwa/NativeHost"
static GDBusConnection *bus(GError **error) {
    return g_bus_get_sync(G_BUS_TYPE_SESSION, NULL, error);
}
/* 0 = no owner, -1 = bus failure, otherwise primary host PID. */
int tuipwa_owner(void) {
    GError *error = NULL;
    GDBusConnection *c = bus(&error);
    if (!c) { g_clear_error(&error); return -1; }
    GVariant *r = g_dbus_connection_call_sync(c, "org.freedesktop.DBus",
        "/org/freedesktop/DBus", "org.freedesktop.DBus", "GetConnectionUnixProcessID",
        g_variant_new("(s)", HOST), G_VARIANT_TYPE("(u)"), G_DBUS_CALL_FLAGS_NO_AUTO_START,
        -1, NULL, &error);
    guint32 pid = 0;
    if (r) { g_variant_get(r, "(u)", &pid); g_variant_unref(r); }
    else if (!g_error_matches(error, G_DBUS_ERROR, G_DBUS_ERROR_NAME_HAS_NO_OWNER) &&
             !g_error_matches(error, G_DBUS_ERROR, G_DBUS_ERROR_SERVICE_UNKNOWN)) pid = (guint32)-1;
    g_clear_error(&error);
    g_object_unref(c);
    return (int)pid;
}
/* Address the unique owner, never a replacement host after a shutdown race. */
int tuipwa_activate(const char *const *args, char *message, size_t size) {
    GError *error = NULL;
    GDBusConnection *c = bus(&error);
    GVariant *r = NULL;
    gchar *owner = NULL;
    if (!c) goto done;
    r = g_dbus_connection_call_sync(c, "org.freedesktop.DBus", "/org/freedesktop/DBus",
        "org.freedesktop.DBus", "GetNameOwner", g_variant_new("(s)", HOST),
        G_VARIANT_TYPE("(s)"), G_DBUS_CALL_FLAGS_NO_AUTO_START, -1, NULL, &error);
    if (!r) goto done;
    g_variant_get(r, "(s)", &owner);
    g_variant_unref(r);
    GVariantBuilder parameters, platform;
    g_variant_builder_init(&parameters, G_VARIANT_TYPE("av"));
    g_variant_builder_add(&parameters, "v", g_variant_new_strv(args, -1));
    g_variant_builder_init(&platform, G_VARIANT_TYPE("a{sv}"));
    r = g_dbus_connection_call_sync(c, owner, PATH, "org.gtk.Actions", "Activate",
        g_variant_new("(sava{sv})", "new-window-command", &parameters, &platform),
        NULL, G_DBUS_CALL_FLAGS_NO_AUTO_START, -1, NULL, &error);
 done:
    if (error) snprintf(message, size, "%s", error->message);
    int ok = r != NULL;
    if (r) g_variant_unref(r);
    g_clear_error(&error);
    g_free(owner);
    if (c) g_object_unref(c);
    return ok;
}
