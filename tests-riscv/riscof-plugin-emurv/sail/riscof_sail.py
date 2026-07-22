# Plugin de referencia Sail para RISCOF (RT-3).
#
# Adaptado del template sail_cSim de riscof 1.25.3 al binario oficial
# prebuilt de sail-riscv 0.13 (`sail_riscv_sim`, CLI unificada: --rv32,
# --test-signature <archivo>, --signature-granularity, --inst-limit) en vez
# del histórico `riscv_sim_RV32 --test-signature=<archivo>`. Se quitó el
# soporte de cobertura (riscv_isac) — RT-3 no lo usa.
#
# El env (link.ld en 0x8000_0000 + tohost/HTIF) es el del template, sin
# cambios: Sail implementa HTIF y su mapa es el estándar. Las signatures
# son independientes de las direcciones por diseño de la suite.

import os
import logging
import shutil

import riscof.utils as utils
from riscof.pluginTemplate import pluginTemplate

logger = logging.getLogger()


class sail(pluginTemplate):
    __model__ = "sail_c_simulator"
    __version__ = "0.13"

    def __init__(self, *args, **kwargs):
        super().__init__(*args, **kwargs)
        config = kwargs.get('config')
        if config is None:
            logger.error("Config node for sail missing.")
            raise SystemExit(1)
        self.num_jobs = str(config['jobs'] if 'jobs' in config else 1)
        self.pluginpath = os.path.abspath(config['pluginpath'])
        self.sail_exe = os.path.abspath(
            os.path.join(config['PATH'] if 'PATH' in config else "", "sail_riscv_sim"))
        self.isa_spec = os.path.abspath(config['ispec']) if 'ispec' in config else ''
        self.platform_spec = os.path.abspath(config['pspec']) if 'pspec' in config else ''

    def initialise(self, suite, work_dir, archtest_env):
        self.suite = suite
        self.work_dir = work_dir
        self.objdump_cmd = 'riscv64-elf-objdump -D {0} > {1};'
        self.compile_cmd = 'riscv64-elf-gcc -march={0} \
         -static -mcmodel=medany -fvisibility=hidden -nostdlib -nostartfiles\
         -T ' + self.pluginpath + '/env/link.ld\
         -I ' + self.pluginpath + '/env/\
         -I ' + archtest_env

    def build(self, isa_yaml, platform_yaml):
        ispec = utils.load_yaml(isa_yaml)['hart0']
        self.xlen = ('64' if 64 in ispec['supported_xlen'] else '32')
        if self.xlen != '32':
            logger.error("Referencia Sail: solo RV32 en RT-3.")
            raise SystemExit(1)
        self.compile_cmd = self.compile_cmd + ' -mabi=ilp32 '
        for exe in ('riscv64-elf-gcc', 'riscv64-elf-objdump'):
            if shutil.which(exe) is None:
                logger.error(exe + ": no encontrado.")
                raise SystemExit(1)
        if not os.path.isfile(self.sail_exe):
            logger.error(self.sail_exe + ": no existe. Correr scripts/run-riscof.sh setup.")
            raise SystemExit(1)

    def runTests(self, testList, cgf_file=None):
        makefile = os.path.join(self.work_dir, "Makefile." + self.name[:-1])
        if os.path.exists(makefile):
            os.remove(makefile)
        make = utils.makeUtil(makefilePath=makefile)
        make.makeCommand = 'make -k -j' + self.num_jobs

        for file in testList:
            testentry = testList[file]
            test = testentry['test_path']
            test_dir = testentry['work_dir']
            test_name = test.rsplit('/', 1)[1][:-2]
            elf = 'ref.elf'

            march = testentry['isa'].lower()
            for ext in ('_zicsr', '_zifencei'):
                if ext not in march:
                    march += ext

            execute = "@cd " + test_dir + ";"
            cmd = self.compile_cmd.format(march) + ' ' + test + ' -o ' + elf
            execute += cmd + ' -D' + " -D".join(testentry['macros']) + ";"
            execute += self.objdump_cmd.format(elf, 'ref.disass')

            sig_file = os.path.join(test_dir, self.name[:-1] + ".signature")
            execute += ('{0} --rv32 --test-signature {1} --signature-granularity 4 '
                        '--inst-limit 100000000 {2} > {3}.log 2>&1;').format(
                            self.sail_exe, sig_file, elf, test_name)
            make.add_target(execute)

        make.execute_all(self.work_dir)
