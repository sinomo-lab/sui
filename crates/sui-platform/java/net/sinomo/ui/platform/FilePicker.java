package net.sinomo.ui.platform;

import android.app.Activity;
import android.app.Application;
import android.app.Fragment;
import android.content.ActivityNotFoundException;
import android.content.ClipData;
import android.content.Context;
import android.content.Intent;
import android.database.Cursor;
import android.net.Uri;
import android.os.Bundle;
import android.os.ParcelFileDescriptor;
import android.provider.DocumentsContract;
import android.provider.DocumentsContract.Document;
import android.provider.OpenableColumns;
import android.webkit.MimeTypeMap;
import java.io.FileNotFoundException;
import java.util.ArrayList;
import java.util.LinkedHashSet;
import java.util.Locale;

/**
 * Shows the system document picker for SUI's file dialogs.
 *
 * <p>A NativeActivity never sees activity results, so each dialog adds this
 * headless fragment to the activity. The fragment starts the picker, reports
 * the chosen document URIs to native code, and removes itself.
 */
public final class FilePicker extends Fragment {
    // Matches the mode numbers in android_file_dialog.rs.
    private static final int OPEN_FILE = 0;
    private static final int OPEN_FILES = 1;
    private static final int SAVE_FILE = 2;
    private static final int OPEN_FOLDER = 3;

    private static final String TAG = "net.sinomo.ui.platform.FilePicker";
    private static final int REQUEST_CODE = 1;

    private long token;
    private int mode;
    private Intent intent;
    private boolean answered;

    public FilePicker() {}

    /** Reports a dialog's answer: the URIs, or null with an error or when cancelled. */
    private static native void onResult(long token, String[] uris, String error);

    /** Shows a picker. Call on the main thread. */
    static void show(Activity activity, long token, int mode, String[] extensions, String name) {
        StateGuard.install(activity.getApplication());
        FilePicker picker = new FilePicker();
        picker.token = token;
        picker.mode = mode;
        picker.intent = intent(mode, extensions, name);
        activity.getFragmentManager().beginTransaction().add(picker, TAG).commitAllowingStateLoss();
    }

    /** The name a document's provider shows for it, or else the URI's last segment. */
    static String displayName(Context context, String uri) {
        Uri parsed = Uri.parse(uri);
        try (Cursor cursor =
                context.getContentResolver()
                        .query(
                                parsed,
                                new String[] {OpenableColumns.DISPLAY_NAME},
                                null,
                                null,
                                null)) {
            if (cursor != null && cursor.moveToFirst() && !cursor.isNull(0)) {
                return cursor.getString(0);
            }
        }
        String segment = parsed.getLastPathSegment();
        return segment == null ? uri : segment;
    }

    /**
     * The documents directly inside a folder from a document tree, as a flat
     * list of URI, display name, and MIME type triples.
     */
    static String[] children(Context context, String folderUri) throws FileNotFoundException {
        Uri folder = Uri.parse(folderUri);
        Uri children =
                DocumentsContract.buildChildDocumentsUriUsingTree(
                        folder, DocumentsContract.getDocumentId(folder));
        String[] columns = {
            Document.COLUMN_DOCUMENT_ID, Document.COLUMN_DISPLAY_NAME, Document.COLUMN_MIME_TYPE
        };
        ArrayList<String> entries = new ArrayList<>();
        try (Cursor cursor = context.getContentResolver().query(children, columns, null, null, null)) {
            if (cursor == null) {
                throw new FileNotFoundException("could not list " + folderUri);
            }
            while (cursor.moveToNext()) {
                entries.add(
                        DocumentsContract.buildDocumentUriUsingTree(folder, cursor.getString(0))
                                .toString());
                entries.add(cursor.getString(1));
                entries.add(cursor.getString(2));
            }
        }
        return entries.toArray(new String[0]);
    }

    /** Creates a document or folder in a folder from a document tree and returns its URI. */
    static String create(Context context, String folderUri, String name, boolean folder)
            throws FileNotFoundException {
        String type = folder ? Document.MIME_TYPE_DIR : mimeType(extension(name));
        Uri created =
                DocumentsContract.createDocument(
                        context.getContentResolver(),
                        Uri.parse(folderUri),
                        type == null ? "application/octet-stream" : type,
                        name);
        if (created == null) {
            throw new FileNotFoundException("could not create " + name);
        }
        return created.toString();
    }

    /** Opens a document and returns a file descriptor that the caller then owns. */
    static int open(Context context, String uri, String mode) throws FileNotFoundException {
        ParcelFileDescriptor descriptor =
                context.getContentResolver().openFileDescriptor(Uri.parse(uri), mode);
        if (descriptor == null) {
            throw new FileNotFoundException("no file descriptor for " + uri);
        }
        return descriptor.detachFd();
    }

    @Override
    public void onCreate(Bundle state) {
        super.onCreate(state);
        try {
            startActivityForResult(intent, REQUEST_CODE);
        } catch (ActivityNotFoundException error) {
            answer(null, "no app on this device can pick documents");
        }
    }

    @Override
    public void onActivityResult(int requestCode, int resultCode, Intent data) {
        if (requestCode != REQUEST_CODE) {
            return;
        }
        answer(resultCode == Activity.RESULT_OK ? uris(mode, data) : null, null);
    }

    @Override
    public void onDestroy() {
        super.onDestroy();
        answer(null, "the activity closed before the file dialog answered");
    }

    private void answer(String[] uris, String error) {
        if (answered) {
            return;
        }
        answered = true;
        onResult(token, uris, error);
        if (isAdded()) {
            getFragmentManager().beginTransaction().remove(this).commitAllowingStateLoss();
        }
    }

    private static Intent intent(int mode, String[] extensions, String name) {
        if (mode == OPEN_FOLDER) {
            return new Intent(Intent.ACTION_OPEN_DOCUMENT_TREE);
        }
        Intent intent =
                new Intent(
                        mode == SAVE_FILE
                                ? Intent.ACTION_CREATE_DOCUMENT
                                : Intent.ACTION_OPEN_DOCUMENT);
        intent.addCategory(Intent.CATEGORY_OPENABLE);
        String[] types = mimeTypes(extensions);
        if (mode == SAVE_FILE) {
            // The provider adds the type's extension when the name lacks one.
            String type = name == null ? null : mimeType(extension(name));
            if (type == null) {
                type = types.length > 0 ? types[0] : "application/octet-stream";
            }
            intent.setType(type);
            if (name != null) {
                intent.putExtra(Intent.EXTRA_TITLE, name);
            }
        } else {
            intent.setType(types.length == 1 ? types[0] : "*/*");
            if (types.length > 1) {
                intent.putExtra(Intent.EXTRA_MIME_TYPES, types);
            }
            intent.putExtra(Intent.EXTRA_ALLOW_MULTIPLE, mode == OPEN_FILES);
        }
        return intent;
    }

    /**
     * The MIME types of the extensions. The picker filters by type, so when one
     * extension has no known type this returns none, and every file shows.
     */
    private static String[] mimeTypes(String[] extensions) {
        LinkedHashSet<String> types = new LinkedHashSet<>();
        for (String extension : extensions) {
            String type = mimeType(extension);
            if (type == null) {
                return new String[0];
            }
            types.add(type);
        }
        return types.toArray(new String[0]);
    }

    private static String mimeType(String extension) {
        if (extension == null || extension.isEmpty()) {
            return null;
        }
        return MimeTypeMap.getSingleton()
                .getMimeTypeFromExtension(extension.toLowerCase(Locale.ROOT));
    }

    private static String extension(String name) {
        int dot = name.lastIndexOf('.');
        return dot < 0 ? null : name.substring(dot + 1);
    }

    private static String[] uris(int mode, Intent data) {
        if (data == null) {
            return new String[0];
        }
        if (mode == OPEN_FOLDER) {
            // A tree URI names the grant; its root document names the folder.
            Uri tree = data.getData();
            if (tree == null) {
                return new String[0];
            }
            return new String[] {
                DocumentsContract.buildDocumentUriUsingTree(
                                tree, DocumentsContract.getTreeDocumentId(tree))
                        .toString()
            };
        }
        ClipData clip = data.getClipData();
        if (clip != null) {
            String[] uris = new String[clip.getItemCount()];
            for (int index = 0; index < uris.length; index++) {
                uris[index] = clip.getItemAt(index).getUri().toString();
            }
            return uris;
        }
        Uri uri = data.getData();
        return uri == null ? new String[0] : new String[] {uri.toString()};
    }

    /**
     * Keeps an activity from saving open pickers. The activity would recreate
     * saved fragments through its own class loader, which cannot see this
     * class, so a picker still open when the process died would crash the next
     * launch.
     */
    private static final class StateGuard implements Application.ActivityLifecycleCallbacks {
        private static boolean installed;

        static void install(Application application) {
            if (installed) {
                return;
            }
            installed = true;
            application.registerActivityLifecycleCallbacks(new StateGuard());
        }

        @Override
        public void onActivitySaveInstanceState(Activity activity, Bundle state) {
            if (activity.getFragmentManager().findFragmentByTag(TAG) != null) {
                state.remove("android:fragments");
            }
        }

        @Override
        public void onActivityCreated(Activity activity, Bundle state) {}

        @Override
        public void onActivityStarted(Activity activity) {}

        @Override
        public void onActivityResumed(Activity activity) {}

        @Override
        public void onActivityPaused(Activity activity) {}

        @Override
        public void onActivityStopped(Activity activity) {}

        @Override
        public void onActivityDestroyed(Activity activity) {}
    }
}
