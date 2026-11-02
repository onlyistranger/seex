#define WIN32_LEAN_AND_MEAN
#include <windows.h>
#include <d3dcompiler.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

int main(int argc, char **argv) {
    const char *target = NULL;
    const char *entry = NULL;
    const char *out_header = NULL;
    const char *var_name = "g_ShaderBytes";
    const char *shader_file = NULL;
    UINT flags = D3DCOMPILE_ENABLE_STRICTNESS;

    for (int i = 1; i < argc; i++) {
        const char *arg = argv[i];
        if (arg[0] == '/' || arg[0] == '-') {
            const char *opt = arg + 1;
            if (strcmp(opt, "T") == 0 && i + 1 < argc) {
                target = argv[++i];
            } else if (strcmp(opt, "E") == 0 && i + 1 < argc) {
                entry = argv[++i];
            } else if (strcmp(opt, "Fh") == 0 && i + 1 < argc) {
                out_header = argv[++i];
            } else if (strcmp(opt, "Vn") == 0 && i + 1 < argc) {
                var_name = argv[++i];
            } else if (strcmp(opt, "O3") == 0) {
                flags |= D3DCOMPILE_OPTIMIZATION_LEVEL3;
            } else if (strcmp(opt, "O2") == 0) {
                flags |= D3DCOMPILE_OPTIMIZATION_LEVEL2;
            } else if (strcmp(opt, "O1") == 0) {
                flags |= D3DCOMPILE_OPTIMIZATION_LEVEL1;
            } else if (strcmp(opt, "O0") == 0 || strcmp(opt, "Od") == 0) {
                flags |= D3DCOMPILE_SKIP_OPTIMIZATION;
            }
        } else {
            shader_file = arg;
        }
    }

    if (!shader_file || !target || !entry) {
        fprintf(stderr, "fxc: missing required arguments (shader file, target, or entry point)\n");
        return 1;
    }

    int wlen = MultiByteToWideChar(CP_UTF8, 0, shader_file, -1, NULL, 0);
    if (wlen <= 0) {
        fprintf(stderr, "fxc: invalid path encoding: %s\n", shader_file);
        return 1;
    }
    WCHAR *wshader_file = (WCHAR *)malloc(wlen * sizeof(WCHAR));
    MultiByteToWideChar(CP_UTF8, 0, shader_file, -1, wshader_file, wlen);

    ID3DBlob *code = NULL;
    ID3DBlob *errors = NULL;

    HRESULT hr = D3DCompileFromFile(
        wshader_file,
        NULL,
        D3D_COMPILE_STANDARD_FILE_INCLUDE,
        entry,
        target,
        flags,
        0,
        &code,
        &errors
    );
    free(wshader_file);

    if (FAILED(hr)) {
        if (errors) {
            fprintf(stderr, "%s\n", (char *)errors->lpVtbl->GetBufferPointer(errors));
            errors->lpVtbl->Release(errors);
        } else {
            fprintf(stderr, "fxc: D3DCompileFromFile failed with HRESULT 0x%08lx\n", hr);
        }
        return 1;
    }

    if (out_header) {
        FILE *f = fopen(out_header, "wb");
        if (!f) {
            fprintf(stderr, "fxc: failed to open output header file '%s'\n", out_header);
            code->lpVtbl->Release(code);
            return 1;
        }

        fprintf(f, "const BYTE %s[] =\n{\n", var_name);
        const unsigned char *bytes = (const unsigned char *)code->lpVtbl->GetBufferPointer(code);
        size_t size = (size_t)code->lpVtbl->GetBufferSize(code);

        for (size_t i = 0; i < size; i++) {
            fprintf(f, "%4u", (unsigned int)bytes[i]);
            if (i + 1 < size) {
                fprintf(f, ",");
            }
            if ((i + 1) % 16 == 0 || i + 1 == size) {
                fprintf(f, "\n");
            }
        }
        fprintf(f, "};\n");
        fclose(f);
    }

    code->lpVtbl->Release(code);
    return 0;
}

